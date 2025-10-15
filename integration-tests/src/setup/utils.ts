import { createPublicClient, createWalletClient, defineChain, formatEther, getContract, Hex, http, parseEther, parseEventLogs, toHex } from 'viem'
import { ADMIN, MNEMONIC } from './constants.js'
import { getContractAddress } from './deploy.js'
import { mnemonicToAccount } from 'viem/accounts'
import type { TypedDocumentString } from '../graphql/graphql.js'
import { graphql } from '../graphql/gql.js'
import { abi } from './abi'


const local = defineChain({
  id: 31337,
  name: 'Local intuition',
  nativeCurrency: {
    decimals: 18,
    name: 'Local Trust',
    symbol: 'lTRUST',
  },
  rpcUrls: {
    default: { http: ['http://localhost:8545'] },
  },
})

export const publicClient = createPublicClient({
  chain: local,
  transport: http('http://localhost:8545'),
})

export const adminClient = createWalletClient({
  chain: local,
  transport: http('http://localhost:8545'),
  account: ADMIN,
})

export async function getIntuition(accountIndex: number) {
  const account = mnemonicToAccount(
    MNEMONIC,
    { accountIndex },
  )

  const address = await getContractAddress()

  const wallet = createWalletClient({
    chain: local,
    transport: http(),
    account: account,
  })
  // balance
  const balance = await publicClient.getBalance({ address: account.address })
  console.log(`Balance: ${parseFloat(formatEther(balance)).toFixed(6)} lTRUST, account: ${account.address}`)

  if (balance.valueOf() < parseEther('10').valueOf()) {
    console.log(`Sending 100 lTRUST to ${account.address}...`)

    // Faucet
    //@ts-ignore
    const hash = await adminClient.sendTransaction({
      account: ADMIN,
      value: parseEther('100'),
      to: account.address,
    })

    await publicClient.waitForTransactionReceipt({ hash })
  }

  const contract = getContract({
    address,
    abi,
    client: {
      public: publicClient,
      wallet: wallet
    }

  })

  async function eventParseAtomCreated(hash: Hex) {
    const { logs, status } = await publicClient.waitForTransactionReceipt({ hash })

    if (status === 'reverted') {
      throw new Error('Transaction reverted')
    }

    const events = parseEventLogs({
      abi,
      logs,
      eventName: 'AtomCreated',
    })

    return events[0].args.termId
  }
  async function eventParseTripleCreated(hash: Hex) {
    const { logs, status } = await publicClient.waitForTransactionReceipt({ hash })

    if (status === 'reverted') {
      throw new Error('Transaction reverted')
    }

    const events = parseEventLogs({
      abi,
      logs,
      eventName: 'TripleCreated',
    })

    return events[0].args.termId
  }

  /*
  * Gets or creates an atom from a URI.
  * If the atom already exists, it returns the existing atom.
  * If the atom does not exist, it creates a new atom and returns the new atom.
  * The atom is created with the minimum deposit.
  */
  async function getOrCreateAtom(uri: string) {

    const vaultId = await contract.read.calculateAtomId([toHex(uri)])
    let atomData = '0x'
    try {
      atomData = await contract.read.getAtom([vaultId])
    } catch { }
    if (atomData !== '0x') {
      console.log(`Atom already exists: ${uri} ${vaultId}`)
      return { vaultId: vaultId, hash: null }
    } else {
      console.log(`Creating atom: ${uri} ...`)
      const { minDeposit } = await contract.read.getGeneralConfig()
      const atomCost = await contract.read.getAtomCost()
      const assets = minDeposit + atomCost
      console.log(`Min deposit: ${minDeposit} wei (${formatEther(minDeposit)} lTRUST),`,
        `atom cost: ${atomCost} wei (${formatEther(atomCost)} lTRUST),`,
        `sending assets: ${assets} wei (${formatEther(assets)} lTRUST)`)
      const hash = await contract.write.createAtoms([[toHex(uri)], [assets]], { value: assets })
      const vaultId = await eventParseAtomCreated(hash)
      console.log(`vaultId: ${vaultId}`)
      await wait(hash)
      return { vaultId, hash }
    }
  }

  async function getCreateOrDepositOnTriple(subjectId: `0x${string}`, predicateId: `0x${string}`, objectId: `0x${string}`, customInitialDeposit?: bigint) {
    const { minDeposit } = await contract.read.getGeneralConfig()
    const tripleCost = await contract.read.getTripleCost()
    const initialDeposit = customInitialDeposit ?? minDeposit

    const tripleId = await contract.read.calculateTripleId([subjectId, predicateId, objectId])
    let tripleExits = false;
    try {
      await contract.read.getTriple([tripleId])
      tripleExits = true
    } catch { }

    if (tripleExits) {
      if (initialDeposit) {
        console.log(`Depositing triple: ${subjectId} ${predicateId} ${objectId} ${initialDeposit} ...`)
        const hash = await contract.write.deposit([wallet.account.address, tripleId, 1n, 0n], { value: initialDeposit })
        await wait(hash)
      }
      return { vaultId: tripleId, hash: null }
    } else {
      console.log(`Creating triple: ${subjectId} ${predicateId} ${objectId} ...`)
      const hash = await contract.write.createTriples([
        [subjectId], [predicateId], [objectId], [tripleCost + initialDeposit]],
        { value: tripleCost + initialDeposit }
      )
      const vaultId = await eventParseTripleCreated(hash)
      console.log(`vaultId: ${vaultId}`)
      await wait(hash)
      return { vaultId, hash }
    }
  }

  return { contract, account, getOrCreateAtom, getCreateOrDepositOnTriple }
}

export async function pinJson(json: any) {
  const apiEndpoint = "http://localhost:3000/upload_json_to_ipfs"
  if (!apiEndpoint) {
    throw new Error('API_ENDPOINT is not set')
  }
  const response = await fetch(apiEndpoint, {
    method: 'POST',
    body: JSON.stringify(json),
    headers: {
      'Content-Type': 'application/json',
    },
  })
  const data = await response.json()
  return `ipfs://${data.Hash}`
}

export enum SystemAtom {
  Person = 'https://schema.org/Person',
  Organization = 'https://schema.org/Organization',
  Thing = 'https://schema.org/Thing',
  FollowAction = 'https://schema.org/FollowAction',
  Keywords = 'https://schema.org/keywords',
  Skills = 'https://schema.org/skills',
  MemberOf = 'https://www.w3.org/ns/org#memberOf',
  WasAssociatedWith = 'https://www.w3.org/ns/prov#wasAssociatedWith'
}


export async function execute<TResult, TVariables>(
  query: TypedDocumentString<TResult, TVariables>,
  ...[variables]: TVariables extends Record<string, never> ? [] : [TVariables]
) {
  const response = await fetch('http://localhost:8080/v1/graphql', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/graphql-response+json'
    },
    body: JSON.stringify({
      query,
      variables
    })
  })

  if (!response.ok) {
    throw new Error('Network response was not ok')
  }
  const json: { data: TResult } = await response.json()

  return json.data
}

export async function wait(hash: string | null) {
  // await new Promise(resolve => setTimeout(resolve, 1000));
  // return true
  if (hash === null) {
    return
  }
  const promise = new Promise(async (resolve, reject) => {
    let count = 0
    console.log(`Waiting 1 sec for transaction http://localhost/tx/${hash}`)
    while (true) {
      await new Promise(resolve => setTimeout(resolve, 1000));
      const data = await execute(graphql(`
        query GetTransactionEvents($hash: String!) {
          events(where: { transaction_hash: { _eq: $hash } }) {
            transaction_hash
          }
        }
      `), { hash });
      if (data?.events.length > 0) {
        return resolve(true);
      }
      count++
      if (count > 10) {
        return reject(new Error('Transaction not found'))
      }
      console.log(`Retry: ${count}`)
    }
  });
  return promise;
}


export function getAbsoluteTripleId(vaultId: bigint): bigint {
  const max = (BigInt(2) ** BigInt(255) * BigInt(2) - BigInt(1)) / BigInt(2)
  const isCounterVault = max < BigInt(vaultId)
  let result = vaultId
  if (isCounterVault) {
    result = BigInt(2) ** BigInt(255) * BigInt(2) - BigInt(1) - BigInt(vaultId)
  }

  return result
}

export function getCounterVaultId(vaultId: bigint): bigint {
  const max = (BigInt(2) ** BigInt(255) * BigInt(2) - BigInt(1))
  return max - vaultId
}
