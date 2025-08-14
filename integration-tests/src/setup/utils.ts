import { createPublicClient, createWalletClient, defineChain, formatEther, getContract, Hex, http, parseEther, parseEventLogs, publicActions, toHex } from 'viem'
import { baseSepolia } from 'viem/chains'
import { ADMIN, MNEMONIC } from './constants.js'
import { getContractAddress } from './deploy.js'
import { mnemonicToAccount } from 'viem/accounts'
import { Multivault } from '@0xintuition/protocol'
import type { TypedDocumentString } from '../graphql/graphql.js'
import { graphql } from '../graphql/gql.js'
import { abi } from './abi'
import { tokenAbi } from './tokenAbi'

const local = defineChain({
  id: 1337,
  name: 'Localhost',
  nativeCurrency: {
    decimals: 18,
    name: 'Ether',
    symbol: 'ETH',
  },
  rpcUrls: {
    default: { http: ['http://127.0.0.1:8545'] },
  },
})

export const publicClient = createPublicClient({
  chain: local,
  transport: http(),
})

export const adminClient = createWalletClient({
  chain: local,
  transport: http(),
  account: ADMIN.address,
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
  console.log(`Balance: ${parseFloat(formatEther(balance)).toFixed(6)} ETH, account: ${account.address}`)

  if (balance.valueOf() < parseEther('0.1').valueOf()) {
    console.log(`Sending 1 ETH to ${account.address}...`)

    // Faucet
    //@ts-ignore
    const hash = await adminClient.sendTransaction({
      account: ADMIN.address,
      value: parseEther('1'),
      to: account.address,
    })

    await publicClient.waitForTransactionReceipt({ hash })
    console.log('Transfering mocktokens')

    await adminClient.writeContract({
      account: ADMIN.address,
      address: '0x70AC54941671aa51008e4224264187A779aa672e',
      abi: tokenAbi,
      functionName: 'transfer',
      args: [account.address, parseEther('10000')]
    })


    console.log('approving')

    await wallet.writeContract({
      address: '0x1707083E145074fB85cB75dA75A457814A091192',
      abi: tokenAbi,
      functionName: 'approve',
      args: [address, parseEther('1000')],
    })
  }




  const multivault = new Multivault({
    //@ts-ignore
    publicClient: publicClient,
    //@ts-ignore
    walletClient: wallet
  }, address)

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

    const vaultId = await contract.read.getAtomIdFromData([toHex(uri)])
    const atomData = await contract.read.atomData([vaultId])
    console.log('aaaaaaaaa', atomData)
    if (atomData !== '0x') {
      console.log(`Atom already exists: ${uri} ${vaultId}`)
      return { vaultId, hash: null }
    } else {
      console.log(`Creating atom: ${uri} ...`)
      const generalConfig = await contract.read.generalConfig()
      // const { vaultId, hash } = await multivault.createAtom({ uri, initialDeposit: BigInt(generalConfig[3]) })
      const res = await contract.simulate.createAtoms([[], BigInt(generalConfig[3])])
      // const res = await contract.simulate.createAtoms([[toHex(uri)], BigInt(generalConfig[3])])
      const hash = await contract.write.createAtoms([[toHex(uri)], BigInt(generalConfig[3])])
      const vaultId = await eventParseAtomCreated(hash)
      console.log(`vaultId: ${vaultId}`)
      await wait(hash)
      return { vaultId, hash }
    }
  }

  async function getCreateOrDepositOnTriple(subjectId: `0x${string}`, predicateId: `0x${string}`, objectId: `0x${string}`, customInitialDeposit?: bigint) {
    const generalConfig = await contract.read.generalConfig()
    const initialDeposit = customInitialDeposit ?? BigInt(generalConfig[3])

    const vaultId = await contract.read.tripleIdFromAtomIds([subjectId, predicateId, objectId])

    if (vaultId) {
      if (initialDeposit) {
        console.log(`Depositing triple: ${subjectId} ${predicateId} ${objectId} ${initialDeposit} ...`)
        const hash = await contract.write.deposit([wallet.account.address, vaultId, 1n, initialDeposit, 0n])
        await wait(hash)
      }
      return { vaultId, hash: null }
    } else {
      console.log(`Creating triple: ${subjectId} ${predicateId} ${objectId} ...`)
      const hash = await contract.write.createTriples([[subjectId], [predicateId], [objectId], initialDeposit])
      const vaultId = await eventParseTripleCreated(hash)
      console.log(`vaultId: ${vaultId}`)
      await wait(hash)
      return { vaultId, hash }
    }
  }

  return { multivault, account, getOrCreateAtom, getCreateOrDepositOnTriple }
}


// export async function getOrCreateAtomWithJson(multivault: Multivault, json: any) {
//   // TODO: Check if the JSON is already pinned
//   const cid = await pinataPinJSON(json)
//   return getOrCreateAtom(multivault, `ipfs://${cid}`)
// }

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
  if (hash === null) {
    return
  }
  const promise = new Promise(async (resolve, reject) => {
    let count = 0
    while (true) {
      console.log(`Waiting for transaction http://localhost/tx/${hash}`)
      console.log(`Count: ${count}`)
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
      await new Promise(resolve => setTimeout(resolve, 1000));
      count++
      if (count > 3600) {
        return reject(new Error('Transaction not found'))
      }
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
