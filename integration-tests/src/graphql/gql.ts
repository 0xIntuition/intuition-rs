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
    "query AtomWithClaims($atomId: numeric!, $address: String) {\n        atom(id: $atomId) {\n          id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        claims(\n          where: { account_id: { _eq: $address }, subject_id: { _eq: $atomId } }\n          order_by: [{ shares: desc }]\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n        claims_from_following(\n          args: { address: $address }\n          where: { subject_id: { _eq: $atomId } }\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n      }": typeof types.AtomWithClaimsDocument,
    "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      ": typeof types.FollowingDocument,
    "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      ": typeof types.GetTransactionEventsDocument,
};
const documents: Documents = {
    "query Atom($atomId: numeric!) {\n        atom(id: $atomId) {\n          label\n        }\n      }": types.AtomDocument,
    "query AtomWithClaims($atomId: numeric!, $address: String) {\n        atom(id: $atomId) {\n          id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        claims(\n          where: { account_id: { _eq: $address }, subject_id: { _eq: $atomId } }\n          order_by: [{ shares: desc }]\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n        claims_from_following(\n          args: { address: $address }\n          where: { subject_id: { _eq: $atomId } }\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n      }": types.AtomWithClaimsDocument,
    "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      ": types.FollowingDocument,
    "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      ": types.GetTransactionEventsDocument,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query Atom($atomId: numeric!) {\n        atom(id: $atomId) {\n          label\n        }\n      }"): typeof import('./graphql').AtomDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query AtomWithClaims($atomId: numeric!, $address: String) {\n        atom(id: $atomId) {\n          id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        claims(\n          where: { account_id: { _eq: $address }, subject_id: { _eq: $atomId } }\n          order_by: [{ shares: desc }]\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n        claims_from_following(\n          args: { address: $address }\n          where: { subject_id: { _eq: $atomId } }\n        ) {\n          predicate {\n            id\n            type\n            label\n          }\n          object {\n            value {\n              thing {\n                name\n                description\n                url\n                image\n              }\n            }\n          }\n        }\n      }"): typeof import('./graphql').AtomWithClaimsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      "): typeof import('./graphql').FollowingDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      "): typeof import('./graphql').GetTransactionEventsDocument;


export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}
