import { expect, test, suite } from 'vitest'
import { execute, getIntuition, pinJson, wait } from './setup/utils.js'
import { graphql } from './graphql/gql.js'

suite('IPFS image caching', async () => {
  const alice = await getIntuition(3)
  const ipfsImageUri = 'ipfs://QmbpYuxCQ3PvaDQMQC7PAehEdxSG124HhRHxhoytnGJ5d8'

  const uri = await pinJson({
    '@context': 'https://schema.org',
    '@type': 'Thing',
    name: 'IPFS Image Test',
    description: 'Testing IPFS image caching functionality',
    image: ipfsImageUri,
  })

  const atom = await alice.getOrCreateAtom(uri)

  expect(atom).toBeDefined()
  expect(atom.vaultId).toBeDefined()

  test('atom is created with IPFS image URL', async () => {
    await wait(atom.hash);

    const result = await execute(
      graphql(`query GetAtom($termId: String!) {
        atom(term_id: $termId) {
          label
          image
        }
      }`),
      { termId: atom.vaultId }
    );

    expect(result).toBeDefined();
    expect(result.atom).toBeDefined();
    expect(result.atom?.label).toBe('IPFS Image Test');
    expect(result.atom?.image).toBe(ipfsImageUri);
  });

  test('cached_image is created after async processing', async () => {
    await wait(atom.hash);

    // Use polling helper (20s timeout, 1s interval)

    const result = await execute(
      graphql(`query GetAtomWithCachedImage($termId: String!) {
        atom(term_id: $termId) {
          image
          cached_image {
            url
            original_url
            safe
            score
            model
            created_at
          }
        }
      }`),
      { termId: atom.vaultId }
    );

    expect(result).toBeDefined();
    expect(result.atom).toBeDefined();

    // Verify cached_image exists and has expected structure
    const cachedImage = result.atom?.cached_image;
    expect(cachedImage).toBeDefined();

    // Verify remote relationship mapping (image -> original_url)
    expect(cachedImage?.original_url).toBe(ipfsImageUri);

    // Verify cached_image has IPFS URL for processed image
    expect(cachedImage?.url).toBeDefined();
    expect(cachedImage?.url).toMatch(/^ipfs:\/\//);

    // Verify classification results
    expect(cachedImage?.safe).toBeDefined();
    expect(typeof cachedImage?.safe).toBe('boolean');

    expect(cachedImage?.model).toBe('Falconsai/nsfw_image_detection');

    expect(cachedImage?.score).toBeDefined();

    expect(cachedImage?.created_at).toBeDefined();

    console.log('Cached image details:', {
      url: cachedImage?.url,
      original_url: cachedImage?.original_url,
      safe: cachedImage?.safe,
      model: cachedImage?.model,
      score: cachedImage?.score
    });
  });
})
