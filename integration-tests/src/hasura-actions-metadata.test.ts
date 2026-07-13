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
