import dotenv from 'dotenv'
import { resolve } from 'path'
import { fileURLToPath } from 'url'

dotenv.config({ path: resolve(fileURLToPath(import.meta.url), '../../../../.env') })

import { createNonceManager, privateKeyToAccount } from 'viem/accounts'
import { jsonRpc } from 'viem/nonce'

export const MNEMONIC =
  'legal winner thank year wave sausage worth useful legal winner thank yellow'
const nonceManager = createNonceManager({
  source: jsonRpc()
})
export const ADMIN = privateKeyToAccount(
  '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80',
  { nonceManager }
) //0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

console.log(`ADMIN: ${ADMIN.address}`)
