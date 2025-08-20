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
    "\n      query GetAgents {\n        triples(where: {\n          _and: [\n            { predicate: { data: { _eq: \"type\" } } },\n            { object: { data: { _eq: \"agent\" } } }\n          ]\n        }) {\n          subject {\n            data\n            claims: as_subject_triples {\n              predicate {\n                data\n              }\n              object {\n                data\n              }\n            }\n          }\n        }\n      }\n      ": typeof types.GetAgentsDocument,
    "\n    query SearchPositions($addresses: _text, $search_fields: jsonb) {\n      positions: search_positions_on_subject(\n        args: {addresses: $addresses, search_fields: $search_fields}\n      ) {\n        term {\n          triple {\n            subject {\n              data\n            }\n            predicate {\n              data\n            }\n            object {\n              data\n            }\n          }\n        }\n      }\n    }\n    ": typeof types.SearchPositionsDocument,
    "query Term($termId: bytea!) {\n        atom(term_id: $termId) {\n          label\n        }\n      }": typeof types.TermDocument,
    "query AtomWithClaims($atomId: bytea!, $address: String) {\n        atom(term_id: $atomId) {\n          term_id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        positions(where: {account_id: {_eq: $address}, term: {triple: {subject_id: {_eq: $atomId}}}}, order_by: {term: {total_market_cap: desc}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n        positions_from_following(args: {address: $address}, where: {term: {triple: {subject_id: {_eq: $atomId}}}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n      }\n      ": typeof types.AtomWithClaimsDocument,
    "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      ": typeof types.FollowingDocument,
    "query AtomOrgProfile($term_id: bytea!) {\n        atom(term_id: $term_id) {\n          term_id\n          label\n          orgs: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/org#memberOf\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          projects: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/prov#wasAssociatedWith\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          skills: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://schema.org/skills\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n        }\n      }": typeof types.AtomOrgProfileDocument,
    "\n      query positions($address: String!) {\n        account(id: $address) {\n          positions {\n            id\n            curve_id\n            term_id\n            shares\n          }\n        }\n      }\n    ": typeof types.PositionsDocument,
    "query SearchTerm($query: String!) {\n        search_term(args: {query: $query}, limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      ": typeof types.SearchTermDocument,
    "query SearchFromFollowing($address: String!, $query: String!) {\n        search_term_from_following(args: {address: $address, query: $query} limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      ": typeof types.SearchFromFollowingDocument,
    "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      ": typeof types.GetTransactionEventsDocument,
    "\n      query atom($id: bytea!) {\n        atom(term_id: $id) {\n          wallet_id\n        }\n      }\n    ": typeof types.AtomDocument,
    "\n      query positions2($address: String!, $term_id: bytea!, $curve_id: numeric!) {\n        positions(where: {account_id: {_eq: $address}, curve_id: {_eq: $curve_id}, term_id: {_eq: $term_id}}) {\n          id\n          curve_id\n          term_id\n          shares\n        }\n      }\n    ": typeof types.Positions2Document,
    "\nquery triple($term_id: bytea!) {\n  triple(term_id: $term_id) {\n    term_id\n    term {\n      total_assets\n      total_market_cap\n    }\n    counter_term {\n      total_assets\n      total_market_cap\n    }\n    triple_term {\n      total_assets\n      total_market_cap\n    }\n  }\n}\n": typeof types.TripleDocument,
};
const documents: Documents = {
    "\n      query GetAgents {\n        triples(where: {\n          _and: [\n            { predicate: { data: { _eq: \"type\" } } },\n            { object: { data: { _eq: \"agent\" } } }\n          ]\n        }) {\n          subject {\n            data\n            claims: as_subject_triples {\n              predicate {\n                data\n              }\n              object {\n                data\n              }\n            }\n          }\n        }\n      }\n      ": types.GetAgentsDocument,
    "\n    query SearchPositions($addresses: _text, $search_fields: jsonb) {\n      positions: search_positions_on_subject(\n        args: {addresses: $addresses, search_fields: $search_fields}\n      ) {\n        term {\n          triple {\n            subject {\n              data\n            }\n            predicate {\n              data\n            }\n            object {\n              data\n            }\n          }\n        }\n      }\n    }\n    ": types.SearchPositionsDocument,
    "query Term($termId: bytea!) {\n        atom(term_id: $termId) {\n          label\n        }\n      }": types.TermDocument,
    "query AtomWithClaims($atomId: bytea!, $address: String) {\n        atom(term_id: $atomId) {\n          term_id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        positions(where: {account_id: {_eq: $address}, term: {triple: {subject_id: {_eq: $atomId}}}}, order_by: {term: {total_market_cap: desc}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n        positions_from_following(args: {address: $address}, where: {term: {triple: {subject_id: {_eq: $atomId}}}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n      }\n      ": types.AtomWithClaimsDocument,
    "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      ": types.FollowingDocument,
    "query AtomOrgProfile($term_id: bytea!) {\n        atom(term_id: $term_id) {\n          term_id\n          label\n          orgs: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/org#memberOf\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          projects: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/prov#wasAssociatedWith\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          skills: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://schema.org/skills\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n        }\n      }": types.AtomOrgProfileDocument,
    "\n      query positions($address: String!) {\n        account(id: $address) {\n          positions {\n            id\n            curve_id\n            term_id\n            shares\n          }\n        }\n      }\n    ": types.PositionsDocument,
    "query SearchTerm($query: String!) {\n        search_term(args: {query: $query}, limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      ": types.SearchTermDocument,
    "query SearchFromFollowing($address: String!, $query: String!) {\n        search_term_from_following(args: {address: $address, query: $query} limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      ": types.SearchFromFollowingDocument,
    "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      ": types.GetTransactionEventsDocument,
    "\n      query atom($id: bytea!) {\n        atom(term_id: $id) {\n          wallet_id\n        }\n      }\n    ": types.AtomDocument,
    "\n      query positions2($address: String!, $term_id: bytea!, $curve_id: numeric!) {\n        positions(where: {account_id: {_eq: $address}, curve_id: {_eq: $curve_id}, term_id: {_eq: $term_id}}) {\n          id\n          curve_id\n          term_id\n          shares\n        }\n      }\n    ": types.Positions2Document,
    "\nquery triple($term_id: bytea!) {\n  triple(term_id: $term_id) {\n    term_id\n    term {\n      total_assets\n      total_market_cap\n    }\n    counter_term {\n      total_assets\n      total_market_cap\n    }\n    triple_term {\n      total_assets\n      total_market_cap\n    }\n  }\n}\n": types.TripleDocument,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n      query GetAgents {\n        triples(where: {\n          _and: [\n            { predicate: { data: { _eq: \"type\" } } },\n            { object: { data: { _eq: \"agent\" } } }\n          ]\n        }) {\n          subject {\n            data\n            claims: as_subject_triples {\n              predicate {\n                data\n              }\n              object {\n                data\n              }\n            }\n          }\n        }\n      }\n      "): typeof import('./graphql').GetAgentsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n    query SearchPositions($addresses: _text, $search_fields: jsonb) {\n      positions: search_positions_on_subject(\n        args: {addresses: $addresses, search_fields: $search_fields}\n      ) {\n        term {\n          triple {\n            subject {\n              data\n            }\n            predicate {\n              data\n            }\n            object {\n              data\n            }\n          }\n        }\n      }\n    }\n    "): typeof import('./graphql').SearchPositionsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query Term($termId: bytea!) {\n        atom(term_id: $termId) {\n          label\n        }\n      }"): typeof import('./graphql').TermDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query AtomWithClaims($atomId: bytea!, $address: String) {\n        atom(term_id: $atomId) {\n          term_id\n          label\n          value {\n            thing {\n              name\n              description\n              url\n              image\n            }\n          }\n        }\n        positions(where: {account_id: {_eq: $address}, term: {triple: {subject_id: {_eq: $atomId}}}}, order_by: {term: {total_market_cap: desc}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n        positions_from_following(args: {address: $address}, where: {term: {triple: {subject_id: {_eq: $atomId}}}}) {\n          term {\n            triple {\n              predicate {\n                term_id\n                type\n                label\n              }\n              object {\n                value {\n                  thing {\n                    name\n                    description\n                    url\n                    image\n                  }\n                }\n              }\n            }\n          }\n        }\n      }\n      "): typeof import('./graphql').AtomWithClaimsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query Following($address: String!) {\n        following(args: {address: $address}) {\n          id\n          atom_id\n        }\n      }\n      "): typeof import('./graphql').FollowingDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query AtomOrgProfile($term_id: bytea!) {\n        atom(term_id: $term_id) {\n          term_id\n          label\n          orgs: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/org#memberOf\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          projects: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://www.w3.org/ns/prov#wasAssociatedWith\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n          skills: as_subject_triples(where: {\n            predicate: {data: {_eq: \"https://schema.org/skills\"}}\n          }) {\n            object {\n              term_id\n              label\n            }\n          }\n        }\n      }"): typeof import('./graphql').AtomOrgProfileDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n      query positions($address: String!) {\n        account(id: $address) {\n          positions {\n            id\n            curve_id\n            term_id\n            shares\n          }\n        }\n      }\n    "): typeof import('./graphql').PositionsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query SearchTerm($query: String!) {\n        search_term(args: {query: $query}, limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      "): typeof import('./graphql').SearchTermDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "query SearchFromFollowing($address: String!, $query: String!) {\n        search_term_from_following(args: {address: $address, query: $query} limit: 2) {\n          id\n          atom {\n            label\n          }\n        }\n      }\n      "): typeof import('./graphql').SearchFromFollowingDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n        query GetTransactionEvents($hash: String!) {\n          events(where: { transaction_hash: { _eq: $hash } }) {\n            transaction_hash\n          }\n        }\n      "): typeof import('./graphql').GetTransactionEventsDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n      query atom($id: bytea!) {\n        atom(term_id: $id) {\n          wallet_id\n        }\n      }\n    "): typeof import('./graphql').AtomDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n      query positions2($address: String!, $term_id: bytea!, $curve_id: numeric!) {\n        positions(where: {account_id: {_eq: $address}, curve_id: {_eq: $curve_id}, term_id: {_eq: $term_id}}) {\n          id\n          curve_id\n          term_id\n          shares\n        }\n      }\n    "): typeof import('./graphql').Positions2Document;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\nquery triple($term_id: bytea!) {\n  triple(term_id: $term_id) {\n    term_id\n    term {\n      total_assets\n      total_market_cap\n    }\n    counter_term {\n      total_assets\n      total_market_cap\n    }\n    triple_term {\n      total_assets\n      total_market_cap\n    }\n  }\n}\n"): typeof import('./graphql').TripleDocument;


export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}
