import { type Address } from 'viem'

export async function getContractAddress(): Promise<Address> {
  if (!process.env.VITE_INTUITION_CONTRACT_ADDRESS) {
    throw new Error('VITE_INTUITION_CONTRACT_ADDRESS is not set')
  }
  return process.env.VITE_INTUITION_CONTRACT_ADDRESS as Address
}
