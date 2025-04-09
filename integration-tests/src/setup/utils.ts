import { createPublicClient, createWalletClient, defineChain, formatEther, http, parseEther } from 'viem'
import { ADMIN, MNEMONIC } from './constants.js'
import { getOrDeployAndInit } from './deploy.js'
import { mnemonicToAccount } from 'viem/accounts'
import { Multivault } from '@0xintuition/protocol'
import type { TypedDocumentString } from '../graphql/graphql.js'
import { graphql } from '../graphql/gql.js'

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
  account: ADMIN,
})

export async function getIntuition(accountIndex: number) {
  const account = mnemonicToAccount(
    MNEMONIC,
    { accountIndex },
  )

  const address = await getOrDeployAndInit()

  // balance
  const balance = await publicClient.getBalance({ address: account.address })
  console.log(`Balance: ${parseFloat(formatEther(balance)).toFixed(6)} ETH, account: ${account.address}`)

  if (balance.valueOf() < parseEther('0.01').valueOf()) {
    console.log(`Fauceting 0.01 ETH to ${account.address}...`)

    // Faucet
    //@ts-ignore
    const hash = await adminClient.sendTransaction({
      account: ADMIN,
      value: parseEther('0.01'),
      to: account.address,
    })

    await publicClient.waitForTransactionReceipt({ hash })
  }
  const wallet = createWalletClient({
    chain: local,
    transport: http(),
    account,
  })

  const multivault = new Multivault({
    //@ts-ignore
    publicClient: publicClient,
    //@ts-ignore
    walletClient: wallet
  }, address)

  async function getOrCreateAtom(uri: string) {
    const vaultId = await multivault.getVaultIdFromUri(uri)
    if (vaultId) {
      return { vaultId, hash: null }
    } else {
      console.log(`Creating atom: ${uri} ...`)
      const { vaultId, hash } = await multivault.createAtom({ uri })
      console.log(`vaultId: ${vaultId}`)
      return { vaultId, hash }
    }
  }

  async function getCreateOrDepositOnTriple(subjectId: bigint, predicateId: bigint, objectId: bigint, initialDeposit?: bigint) {

    const vaultId = await multivault.getTripleIdFromAtoms(subjectId, predicateId, objectId)
    if (vaultId) {
      if (initialDeposit) {
        await multivault.depositTriple(vaultId, initialDeposit)
      }
      return { vaultId, hash: null }
    } else {
      console.log(`Creating triple: ${subjectId} ${predicateId} ${objectId} ...`)
      const { vaultId, hash } = await multivault.createTriple({ subjectId, predicateId, objectId, initialDeposit })
      console.log(`vaultId: ${vaultId}`)
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

export enum PredicateType {
  Person = 'https://schema.org/Person',
  Organization = 'https://schema.org/Organization',
  Thing = 'https://schema.org/Thing',
  FollowAction = 'https://schema.org/FollowAction',
  Keywords = 'https://schema.org/keywords',
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
      console.log(`Waiting for transaction ${hash}...`)
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
      if (count > 10) {
        return reject(new Error('Transaction not found'))
      }
    }
  });
  return promise;
}