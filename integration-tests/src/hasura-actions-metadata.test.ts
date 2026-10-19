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
  return parseActionBlocks(readFileSync(actionsYamlPath, 'utf8'))
}

function parseActionBlocks(actionsYaml: string): HasuraActionBlock[] {
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
 * Anonymous-access allow-list (ENG-16792). `anonymous` is the unauthenticated
 * Hasura role, so an action that grants it is callable by anyone on the
 * internet. Only the read-only chart / PnL actions below may do so; any other
 * action granting `anonymous` (whatever its handler, including a templated one
 * such as `'{{imageGuardUrl}}'`) fails this guard until a reviewer adds it here
 * on purpose.
 */
const ANONYMOUS_ALLOWED_ACTIONS = [
  'getAccountPnlChart',
  'getAccountPnlCurrent',
  'getAccountPnlRealized',
  'getChartJson',
  'getChartRawJson',
  'getChartRawSvg',
  'getChartSvg',
  'getPositionPnlChart',
  'get_pnl_leaderboard_period',
  'get_pnl_leaderboard_period_min_threshold',
]

/**
 * Actions that proxy to image-guard / Pinata (IPFS pinning and the
 * user-supplied URL fetch). They must be callable by exactly these roles.
 */
const PINNING_ACTIONS = [
  'pinThing',
  'pinPerson',
  'pinOrganization',
  'uploadImage',
  'uploadImageFromUrl',
  'uploadJsonToIpfs',
]
const PINNING_ROLES = ['frontend', 'partner']

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

/** Names of actions that grant `anonymous` without being on the allow-list. */
function unlistedAnonymousActions(blocks: HasuraActionBlock[], allowed: string[]): string[] {
  return blocks
    .filter(({ name, block }) => rolesOf(block).includes('anonymous') && !allowed.includes(name))
    .map(({ name }) => name)
}

test('permission parser reads roles (guards the guard against a silent no-op)', () => {
  const chart = actionBlocks().find(({ name }) => name === 'getChartJson')
  expect(chart, 'getChartJson fixture action must exist').toBeDefined()
  expect(rolesOf(chart!.block)).toEqual(['anonymous', 'frontend'])
})

test('only allow-listed actions grant the anonymous role', () => {
  const blocks = actionBlocks()
  expect(unlistedAnonymousActions(blocks, ANONYMOUS_ALLOWED_ACTIONS)).toEqual([])
  // The allow-list must not rot: every entry still exists and still grants it.
  for (const name of ANONYMOUS_ALLOWED_ACTIONS) {
    const action = blocks.find((b) => b.name === name)
    expect(action, `${name} is allow-listed but missing from actions.yaml`).toBeDefined()
    expect(rolesOf(action!.block), `${name} is allow-listed but no longer anonymous`).toContain('anonymous')
  }
})

test('pinning and upload actions are granted to exactly frontend and partner', () => {
  const blocks = actionBlocks()
  for (const name of PINNING_ACTIONS) {
    const action = blocks.find((b) => b.name === name)
    expect(action, `${name} must exist in actions.yaml`).toBeDefined()
    expect([...rolesOf(action!.block)].sort(), `${name} roles`).toEqual([...PINNING_ROLES].sort())
  }
})

test('the guard rejects a new action with a templated handler that grants anonymous', () => {
  const yaml = `actions:
  - name: getChartJson
    definition:
      handler: '{{chartApiUrl}}'
    permissions:
      - role: anonymous
      - role: frontend
  - name: sneakyUpload
    definition:
      handler: '{{imageGuardUrl}}'
    permissions:
      - role: anonymous
`
  const blocks = parseActionBlocks(yaml)
  expect(blocks.map(({ name }) => name)).toEqual(['getChartJson', 'sneakyUpload'])
  expect(unlistedAnonymousActions(blocks, ANONYMOUS_ALLOWED_ACTIONS)).toEqual(['sneakyUpload'])
})
