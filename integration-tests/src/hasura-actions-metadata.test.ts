import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, test } from 'vitest'

const actionsYamlPath = resolve(
  import.meta.dirname,
  '../../infrastructure/hasura/metadata/actions.yaml',
)

type HasuraActionBlock = {
  name: string
  block: string
}

function actionBlocks(): HasuraActionBlock[] {
  const actionsYaml = readFileSync(actionsYamlPath, 'utf8')
  const markers = [...actionsYaml.matchAll(/^  - name: (.+)$/gm)]

  return markers.map((marker, index) => {
    const start = marker.index ?? 0
    const next = markers[index + 1]?.index ?? actionsYaml.length

    return {
      name: marker[1],
      block: actionsYaml.slice(start, next),
    }
  })
}

function hasJsonContentType(block: string): boolean {
  return /headers:\n\s+- name: Content-Type\n\s+value: application\/json/.test(block)
}

test('image-guard JSON upload actions declare the required content type', () => {
  const imageGuardJsonActions = actionBlocks().filter(({ block }) => {
    return (
      block.includes('handler: http://image-guard:3000/upload_image_from_url')
      && block.includes('request_transform:')
      && block.includes('"url": ')
    )
  })

  expect(imageGuardJsonActions.map(({ name }) => name)).toEqual(
    expect.arrayContaining(['uploadImage', 'uploadImageFromUrl']),
  )
  expect(imageGuardJsonActions.length).toBeGreaterThanOrEqual(2)

  for (const action of imageGuardJsonActions) {
    expect(
      hasJsonContentType(action.block),
      `${action.name} must send application/json to image-guard's JSON endpoint`,
    ).toBe(true)
  }
})

/**
 * Actions that proxy to image-guard / Pinata (anonymous IPFS pinning + SSRF
 * surface, ENG-16792). They must stay restricted to authenticated roles.
 */
const PINNING_ACTIONS = [
  'pinThing',
  'pinPerson',
  'pinOrganization',
  'uploadImage',
  'uploadImageFromUrl',
  'uploadJsonToIpfs',
]
const ALLOWED_PINNING_ROLES = ['frontend', 'partner']

function rolesOf(block: string): string[] {
  const permissions = block.split(/^    permissions:\n/m)[1]
  if (permissions === undefined) return []
  const roles: string[] = []
  for (const line of permissions.split('\n')) {
    const match = line.match(/^      - role: (\S+)\s*$/)
    if (match) roles.push(match[1])
    else if (!/^\s{6}\S|^\s*$/.test(line)) break // left the permissions list
  }
  return roles
}

test('permission parser reads roles (guards the guard against a silent no-op)', () => {
  const chart = actionBlocks().find(({ name }) => name === 'getChartJson')
  expect(chart, 'getChartJson fixture action must exist').toBeDefined()
  expect(rolesOf(chart!.block)).toEqual(['anonymous', 'frontend'])
})

test('pinning and upload actions are never granted to anonymous or unlisted roles', () => {
  const blocks = actionBlocks()
  for (const name of PINNING_ACTIONS) {
    const action = blocks.find((b) => b.name === name)
    expect(action, `${name} must exist in actions.yaml`).toBeDefined()
    const roles = rolesOf(action!.block)
    expect(roles.length, `${name} must declare at least one role`).toBeGreaterThan(0)
    expect(roles, `${name} must not be callable anonymously`).not.toContain('anonymous')
    for (const role of roles) {
      expect(ALLOWED_PINNING_ROLES, `${name} grants unexpected role "${role}"`).toContain(role)
    }
  }
})

test('any action handled by image-guard or Pinata is in the guarded list and never anonymous', () => {
  const sensitive = actionBlocks().filter(({ block }) => {
    return /handler: https?:\/\/(image-guard[:/]|api\.pinata\.cloud)/.test(block)
  })
  // Guard against the matcher silently finding nothing.
  expect(sensitive.map(({ name }) => name).sort()).toEqual(expect.arrayContaining([...PINNING_ACTIONS].sort()))
  for (const { name, block } of sensitive) {
    expect(PINNING_ACTIONS, `${name} proxies to image-guard/Pinata: add it to PINNING_ACTIONS`).toContain(name)
    expect(rolesOf(block), `${name} must not be anonymous`).not.toContain('anonymous')
  }
})
