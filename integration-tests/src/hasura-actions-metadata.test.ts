import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { expect, test } from 'vitest'

const actionsYamlPath = resolve(
  import.meta.dirname,
  '../../infrastructure/hasura/metadata/actions.yaml',
)

function actionBlock(actionName: string): string {
  const actionsYaml = readFileSync(actionsYamlPath, 'utf8')
  const startMarker = `  - name: ${actionName}`
  const start = actionsYaml.indexOf(startMarker)

  expect(start, `${actionName} action should exist`).toBeGreaterThanOrEqual(0)

  const remaining = actionsYaml.slice(start + startMarker.length)
  const nextAction = remaining.search(/\n  - name: /)
  const end = nextAction === -1 ? actionsYaml.length : start + startMarker.length + nextAction

  return actionsYaml.slice(start, end)
}

test('uploadImage action sends JSON to image-guard with the required content type', () => {
  const uploadImage = actionBlock('uploadImage')

  expect(uploadImage).toContain(
    'handler: http://image-guard:3000/upload_image_from_url',
  )
  expect(uploadImage).toContain(
    '"url": "data:{{$body.input.image.contentType}};base64,{{$body.input.image.data}}"',
  )
  expect(uploadImage).toMatch(/headers:\n\s+- name: Content-Type\n\s+value: application\/json/)
})
