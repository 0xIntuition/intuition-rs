import { type Address } from 'viem'
import fs from 'fs'

export async function getContractAddress(): Promise<Address> {
  if (!process.env.VITE_CONTRACT_DEPLOYER_INFO_PATH) {
    throw new Error('VITE_CONTRACT_DEPLOYER_INFO_PATH is not set')
  }
  const contractDeployerInfo = JSON.parse(fs.readFileSync(process.env.VITE_CONTRACT_DEPLOYER_INFO_PATH, 'utf8'))
  return contractDeployerInfo.transactions.find((t: any) => t.contractName === 'TransparentUpgradeableProxy')?.contractAddress as Address
}
