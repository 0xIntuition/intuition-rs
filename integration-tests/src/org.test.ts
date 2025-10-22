import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, SystemAtom, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('organization, projects, people', async () => {

  const admin = await getIntuition(1)

  // System atoms

  const person = await admin.getOrCreateAtom(
    SystemAtom.Person,
  )

  const skills = await admin.getOrCreateAtom(
    SystemAtom.Skills,
  )

  const memberOf = await admin.getOrCreateAtom(
    SystemAtom.MemberOf,
  )

  const wasAssociatedWith = await admin.getOrCreateAtom(
    SystemAtom.WasAssociatedWith,
  )

  const hasTag = await admin.getOrCreateAtom(
    SystemAtom.Keywords,
  )
  // People

  // Maya

  const maya = await getIntuition(11)

  const mayaAccount = await maya.getOrCreateAtom(
    maya.account.address
  )

  const mayaPerson = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Person',
    name: 'Maya',
    description: 'Product manager passionate about building great products',
    email: 'maya@nova-biotech.example',
    url: 'https://nova-biotech.example',
  }))

  const triple = await maya.getCreateOrDepositOnTriple(
    mayaAccount.vaultId,
    person.vaultId,
    mayaPerson.vaultId,
  )

  expect(person).toBeDefined()
  expect(mayaAccount).toBeDefined()
  expect(mayaPerson).toBeDefined()
  expect(triple.vaultId).toBeDefined()

  // Leo

  const leo = await getIntuition(12)

  const leoAccount = await leo.getOrCreateAtom(
    leo.account.address
  )

  const leoPerson = await leo.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Person',
    name: 'Leo',
    description: 'Software developer passionate about building great products',
    email: 'leo@acme.example',
  }))

  const leoTriple = await leo.getCreateOrDepositOnTriple(
    leoAccount.vaultId,
    person.vaultId,
    leoPerson.vaultId,
  )

  expect(leoPerson).toBeDefined()
  expect(leoAccount).toBeDefined()
  expect(leoTriple.vaultId).toBeDefined()


  // Organizations

  // Nova Biotech
  const novaBiotechOrg = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: 'Nova Biotech',
    description: 'A company that builds great products',
    url: 'https://nova-biotech.example',
  }))

  // GridSec
  const gridSecOrg = await leo.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: 'GridSec',
    description: 'A company that builds great products',
    url: 'https://gridsec.example',
  }))

  // SkyChain
  const skyChainOrg = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Organization',
    name: 'SkyChain',
    description: 'A company that builds great products',
    url: 'https://skychain.example',
  }))

  // Projects

  // Helix
  const helixProject = await leo.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Helix',
    description: 'Great project that makes the world a better place',
    url: 'https://helix.example',
  }))

  // Sentinel
  const sentinelProject = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Sentinel',
    description: 'Great project that makes sense',
    url: 'https://sentinel.example',
  }))

  // Organizations and projects

  // Nova Biotech
  const novaBiotechHelix = await leo.getCreateOrDepositOnTriple(
    novaBiotechOrg.vaultId,
    wasAssociatedWith.vaultId,
    helixProject.vaultId,
  )

  const skyChainSentinel = await maya.getCreateOrDepositOnTriple(
    skyChainOrg.vaultId,
    wasAssociatedWith.vaultId,
    sentinelProject.vaultId,
  )

  // GridSec
  const gridSecHelix = await leo.getCreateOrDepositOnTriple(
    gridSecOrg.vaultId,
    wasAssociatedWith.vaultId,
    helixProject.vaultId,
  )

  // People and organizations
  // Maya
  const mayaNovaBiotech = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    memberOf.vaultId,
    novaBiotechOrg.vaultId,
  )

  const mayaSkyChain = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    memberOf.vaultId,
    skyChainOrg.vaultId,
  )

  // Leo
  const leoNovaBiotech = await leo.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    memberOf.vaultId,
    novaBiotechOrg.vaultId,
  )

  const leoGridSec = await leo.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    memberOf.vaultId,
    gridSecOrg.vaultId,
  )

  // People and projects
  // Maya
  const mayaHelix = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    wasAssociatedWith.vaultId,
    helixProject.vaultId,
  )

  const mayaSentinel = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    wasAssociatedWith.vaultId,
    sentinelProject.vaultId,
  )

  // Leo
  const leoHelix = await leo.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    wasAssociatedWith.vaultId,
    helixProject.vaultId,
  )

  const leoSentinel = await leo.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    wasAssociatedWith.vaultId,
    sentinelProject.vaultId,
  )

  // People and skills
  // Developer
  const developerSkill = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Developer',
    description: 'A developer is a person who develops great products',
  }))

  // Product manager
  const productManagerSkill = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Product Manager',
    description: 'A product manager is a person who manages great products',
  }))

  // Designer
  const designerSkill = await maya.getOrCreateAtom(await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'Designer',
    description: 'A designer is a person who designs great products',
  }))

  // People and skills
  // Maya
  const mayaDeveloper = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    skills.vaultId,
    developerSkill.vaultId,
  )

  const mayaProductManager = await maya.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    skills.vaultId,
    productManagerSkill.vaultId,
  )

  // Leo
  const leoDeveloper = await leo.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    skills.vaultId,
    developerSkill.vaultId,
  )


  // Tags

  const web3Tag = await admin.getOrCreateAtom('Web3')
  const manufacturingTag = await admin.getOrCreateAtom('Manufacturing')

  // Industry/Domain tags
  const biotechnologyTag = await admin.getOrCreateAtom('Biotechnology')
  const cybersecurityTag = await admin.getOrCreateAtom('Cybersecurity')
  const blockchainTag = await admin.getOrCreateAtom('Blockchain')
  const supplyChainTag = await admin.getOrCreateAtom('Supply Chain')
  const healthcareTag = await admin.getOrCreateAtom('Healthcare')

  // Technology tags
  const aiMlTag = await admin.getOrCreateAtom('AI/ML')
  const cloudComputingTag = await admin.getOrCreateAtom('Cloud Computing')
  const smartContractsTag = await admin.getOrCreateAtom('Smart Contracts')

  // Organization type tags
  const startupTag = await admin.getOrCreateAtom('Startup')
  const enterpriseTag = await admin.getOrCreateAtom('Enterprise')

  // Project stage tags
  const productionTag = await admin.getOrCreateAtom('Production')
  const researchTag = await admin.getOrCreateAtom('Research')

  // People attribute tags
  const remoteTag = await admin.getOrCreateAtom('Remote')
  const leadershipTag = await admin.getOrCreateAtom('Leadership')
  const technicalTag = await admin.getOrCreateAtom('Technical')

  // Tag organizations

  // Nova Biotech
  await maya.getCreateOrDepositOnTriple(
    novaBiotechOrg.vaultId,
    hasTag.vaultId,
    biotechnologyTag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    novaBiotechOrg.vaultId,
    hasTag.vaultId,
    healthcareTag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    novaBiotechOrg.vaultId,
    hasTag.vaultId,
    startupTag.vaultId,
  )

  // GridSec
  await maya.getCreateOrDepositOnTriple(
    gridSecOrg.vaultId,
    hasTag.vaultId,
    cybersecurityTag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    gridSecOrg.vaultId,
    hasTag.vaultId,
    enterpriseTag.vaultId,
  )

  // SkyChain
  await maya.getCreateOrDepositOnTriple(
    skyChainOrg.vaultId,
    hasTag.vaultId,
    blockchainTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    skyChainOrg.vaultId,
    hasTag.vaultId,
    supplyChainTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    skyChainOrg.vaultId,
    hasTag.vaultId,
    startupTag.vaultId,
  )

  // Tag projects

  // Helix
  await maya.getCreateOrDepositOnTriple(
    helixProject.vaultId,
    hasTag.vaultId,
    web3Tag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    helixProject.vaultId,
    hasTag.vaultId,
    manufacturingTag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    helixProject.vaultId,
    hasTag.vaultId,
    smartContractsTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    helixProject.vaultId,
    hasTag.vaultId,
    productionTag.vaultId,
  )

  // Sentinel
  await leo.getCreateOrDepositOnTriple(
    sentinelProject.vaultId,
    hasTag.vaultId,
    cybersecurityTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    sentinelProject.vaultId,
    hasTag.vaultId,
    cloudComputingTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    sentinelProject.vaultId,
    hasTag.vaultId,
    productionTag.vaultId,
  )

  // Tag people

  // Maya
  await leo.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    hasTag.vaultId,
    leadershipTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    hasTag.vaultId,
    remoteTag.vaultId,
  )

  await leo.getCreateOrDepositOnTriple(
    mayaPerson.vaultId,
    hasTag.vaultId,
    technicalTag.vaultId,
  )

  // Leo
  await maya.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    hasTag.vaultId,
    technicalTag.vaultId,
  )

  await maya.getCreateOrDepositOnTriple(
    leoPerson.vaultId,
    hasTag.vaultId,
    remoteTag.vaultId,
  )



  expect(mayaDeveloper).toBeDefined()
  expect(mayaProductManager).toBeDefined()
  expect(leoDeveloper).toBeDefined()
  expect(mayaSentinel).toBeDefined()
  expect(leoSentinel).toBeDefined()
  expect(mayaHelix).toBeDefined()
  expect(leoHelix).toBeDefined()
  expect(mayaNovaBiotech).toBeDefined()
  expect(leoNovaBiotech).toBeDefined()
  expect(mayaSkyChain).toBeDefined()
  expect(leoGridSec).toBeDefined()

  test('Org profile', async () => {
    await wait(leoDeveloper.hash)
    const result = await execute(
      graphql(`query AtomOrgProfile($term_id: String!) {
        atom(term_id: $term_id) {
          term_id
          label
          orgs: as_subject_triples(where: {
            predicate: {data: {_eq: "is a member of"}}
          }) {
            object {
              term_id
              label
            }
          }
          projects: as_subject_triples(where: {
            predicate: {data: {_eq: "was associated with"}}
          }) {
            object {
              term_id
              label
            }
          }
          skills: as_subject_triples(where: {
            predicate: {data: {_eq: "is skilled in"}}
          }) {
            object {
              term_id
              label
            }
          }
        }
      }`),
      { term_id: mayaPerson.vaultId }
    )
    expect(result).toBeDefined()
    expect(result.atom?.orgs.length).toBe(2)
    expect(result.atom?.projects.length).toBe(2)
    expect(result.atom?.skills.length).toBe(2)
    expect(result.atom?.label).toBe('Maya')
    expect(result.atom?.orgs.some((org) => org.object.term_id === novaBiotechOrg.vaultId)).toBe(true)
    expect(result.atom?.orgs.some((org) => org.object.term_id === skyChainOrg.vaultId)).toBe(true)
    expect(result.atom?.projects.some((project) => project.object.term_id === helixProject.vaultId)).toBe(true)
    expect(result.atom?.projects.some((project) => project.object.term_id === sentinelProject.vaultId)).toBe(true)
    expect(result.atom?.skills.some((skill) => skill.object.term_id === developerSkill.vaultId)).toBe(true)
    expect(result.atom?.skills.some((skill) => skill.object.term_id === productManagerSkill.vaultId)).toBe(true)
  })

})
