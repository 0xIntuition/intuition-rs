import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'
import { abi } from './setup/abi.js'
import { adminClient, publicClient } from './setup/utils.js'
import { getContractAddress } from './setup/deploy.js'
import { getContract, parseEther, toHex } from 'viem'

suite('migration mode', async () => {
  const address = await getContractAddress()
  const contract = getContract({
    abi,
    address,
    client: {
      public: publicClient,
      wallet: adminClient
    }
  })

  const atoms = ['Earth', 'is', 'round']

  const triple = [
    await contract.read.calculateAtomId([toHex(atoms[0])]),
    await contract.read.calculateAtomId([toHex(atoms[1])]),
    await contract.read.calculateAtomId([toHex(atoms[2])]),
  ] as const

  const tripleId = await contract.read.calculateTripleId([triple[0], triple[1], triple[2]])

  const alice = await getIntuition(10)

  test('should batch set atom data', async () => {
    const hash = await contract.write.batchSetAtomData([
      atoms.map(data => alice.account.address),
      atoms.map(data => toHex(data)),
    ])
    expect(hash).toBeDefined()
    console.log(`http://localhost/tx/${hash}`)
    const receipt = await publicClient.waitForTransactionReceipt({ hash })
    expect(receipt).toBeDefined()
  })

  test('should batch set triple data', async () => {
    const hash = await contract.write.batchSetTripleData([
      [alice.account.address],
      [triple],
    ])
    expect(hash).toBeDefined()
    console.log(`http://localhost/tx/${hash}`)
    const receipt = await publicClient.waitForTransactionReceipt({ hash })
    expect(receipt).toBeDefined()
  })

  test('should batch set vault data', async () => {
    const termIds = [...triple, tripleId]
    const vaultTotals = termIds.map(i => ({
      totalAssets: parseEther('1'),
      totalShares: parseEther('1')
    }))
    const hash = await contract.write.batchSetVaultTotals([
      termIds,
      BigInt(1),
      vaultTotals
    ]);
    expect(hash).toBeDefined()
    console.log(`http://localhost/tx/${hash}`)
    const receipt = await publicClient.waitForTransactionReceipt({ hash })
    expect(receipt).toBeDefined()
  })

  test('should batch set user balances data', async () => {
    const termIds = [...triple, tripleId]
    const userBalances = termIds.map(i => (parseEther('1')))
    const hash = await contract.write.batchSetUserBalances([{
      termIds: [termIds],
      bondingCurveId: BigInt(1),
      users: [alice.account.address],
      userBalances: [userBalances]
    }]);
    expect(hash).toBeDefined()
    console.log(`http://localhost/tx/${hash}`)
    const receipt = await publicClient.waitForTransactionReceipt({ hash })
    expect(receipt).toBeDefined()
  })

  test('should set term count', async () => {
    const termIds = [...triple, tripleId]
    const hash = await contract.write.setTermCount([BigInt(termIds.length)]);
    expect(hash).toBeDefined()
    console.log(`http://localhost/tx/${hash}`)
    const receipt = await publicClient.waitForTransactionReceipt({ hash })
    expect(receipt).toBeDefined()
  })



})
