import { privateKeyToAccount } from 'viem/accounts'

export const MNEMONIC =
  'legal winner thank year wave sausage worth useful legal winner thank yellow'

export const CONTRACT_ADDRESS = '0x04056c43d0498b22f7a0c60d4c3584fb5fa881cc'

export const PROTOCOL_MULTISIG = '0xEcAc3Da134C2e5f492B702546c8aaeD2793965BB'

export const ADMIN = privateKeyToAccount(
  '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
) //0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

console.log(`ADMIN: ${ADMIN.address}`)
