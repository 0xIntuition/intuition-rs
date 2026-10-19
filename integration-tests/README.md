# Integration tests

```
export VITE_INTUITION_CONTRACT_ADDRESS=0x....
pnpm i
pnpm test
```

## Hasura actions metadata guard

`pnpm run test:metadata` runs a static check of
`infrastructure/hasura/metadata/actions.yaml` and needs no running services
(unlike `pnpm test`, which starts the full live-stack suite in watch mode). It
verifies that:

- only the actions on the explicit allow-list in
  `src/hasura-actions-metadata.test.ts` (the read-only chart/PnL actions) grant
  the `anonymous` role. Adding a new action that is callable anonymously fails
  the check until the allow-list is edited on purpose in a reviewed change;
- the pinning and upload actions (`pinThing`, `pinPerson`, `pinOrganization`,
  `uploadImage`, `uploadImageFromUrl`, `uploadJsonToIpfs`) are granted to exactly
  `frontend` and `partner`;
- the image-guard JSON upload actions send `Content-Type: application/json`.

CI runs it on pull requests that touch `infrastructure/hasura/**` or
`integration-tests/**` (`.github/workflows/metadata-guard.yml`).
