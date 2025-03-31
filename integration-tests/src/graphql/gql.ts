/* eslint-disable */
import * as types from './graphql';



/**
 * Map of all GraphQL operations in the project.
 *
 * This map has several performance disadvantages:
 * 1. It is not tree-shakeable, so it will include all operations in the project.
 * 2. It is not minifiable, so the string of a GraphQL query will be multiple times inside the bundle.
 * 3. It does not support dead code elimination, so it will add unused operations.
 *
 * Therefore it is highly recommended to use the babel or swc plugin for production.
 * Learn more about it here: https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#reducing-bundle-size
 */
type Documents = {
    "query Atom($atomId: numeric!) {\n        atom(id: $atomId) {\n          label\n        }\n      }": typeof types.AtomDocument,
    "mutation PinThing($thing: PinThingInput!) {\n  pinThing(thing: $thing) {\n    uri\n  }\n}": typeof types.PinThingDocument,
    "mutation PinPerson($person: PinPersonInput!) {\n  pinPerson(person: $person) {\n    uri\n  }\n}": typeof types.PinPersonDocument,
};
const documents: Documents = {
    "query Atom($atomId: numeric!) {\n        atom(id: $atomId) {\n          label\n        }\n      }": types.AtomDocument,
    "mutation PinThing($thing: PinThingInput!) {\n  pinThing(thing: $thing) {\n    uri\n  }\n}": types.PinThingDocument,
    "mutation PinPerson($person: PinPersonInput!) {\n  pinPerson(person: $person) {\n    uri\n  }\n}": types.PinPersonDocument,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query Atom($atomId: numeric!) {\n        atom(id: $atomId) {\n          label\n        }\n      }"): typeof import('./graphql').AtomDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "mutation PinThing($thing: PinThingInput!) {\n  pinThing(thing: $thing) {\n    uri\n  }\n}"): typeof import('./graphql').PinThingDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "mutation PinPerson($person: PinPersonInput!) {\n  pinPerson(person: $person) {\n    uri\n  }\n}"): typeof import('./graphql').PinPersonDocument;


export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}
