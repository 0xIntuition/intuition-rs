# ArgoCD

## Concepts

Our ArgoCD setup consists of **core apps** which are deployed as single common instances for all environments and **namespaced apps** which are application sets deploying new instances of each app in environment namespaces.

### Core Apps

Core Apps argo manifests are kept in `/devops/argocd/coreapps`, there is an app of apps that watches over them to apply the manifests in the kubernetes cluster.

You can define core apps as Applications or ApplicationSet.

### Namespaced apps

Namespaced apps are apps that semantically or logically separated by their `${env}-${version}`, example `dev-base-sepolia-v200`. They are either deployed in a namespace named `${env}-version` or their name is prefixed by `${env}-${version}` and they are deployed in a shared namespace(this is the case for the graphql api and for cluster level resources only).

The goal is to be able to have several environments running concurrently and potentially several versions of the same environment so we can do blue/green deployments with full reindexation without downtime or a/b testing etc.

Namespaced apps argo manifests are kept in `/devops/argocd/coreapps`, there in an app of apps that watches over them to apply the manifests in the kubernetes cluster.

Namespaced apps are declared using argo ApplicationSet so parameters can be templated according on the environment and version.


## Deploy a new environment

1. Add an element in the main-db-creds namespaced app at `/devops/argocd/namespacedapps/db-main-creds.yaml` for your new environment
  - this will create a secret in the new namespace with the credentials for the main database server

2. Add a new element in the createdb namespaced app at `/devops/argocd/namespacedapps/createdb.yaml` and deploy it
  - this will create a new database for the new environment to be used for the graphql api in the main database server

3. Create a migration to add your environment cursor in histoflux

4. Add a new element in each namespaced app in no particular order, make sure you adjust all parameters accordingly
  - this will create all required apps and their dependencies(including SQS)

5. Create an aws certificate for your domains and add an entry in the ingress at `/devops/argocd/manifests/ingress-main/ingress.yaml`, push changes to git and they will be applied automatically

6. Wait for everything to be ready, you can look at the argocd UI for any issue, when ready you should be able to query your graphql api

## Blue Green deployments

Once your new environment is ready, change the main ingress for the environment to point to the appropriate namespaced service
