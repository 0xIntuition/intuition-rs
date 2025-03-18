#!/bin/bash

ACCOUNT_ID=$(aws sts get-caller-identity \
  --query "Account" --output text)
OIDC_PROVIDER=$(aws eks describe-cluster --name $EKS_CLUSTER_NAME \
  --query "cluster.identity.oidc.issuer" --output text | sed -e 's|^https://||')

cat <<-EOF > trust.json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Principal": {
                "Federated": "arn:aws:iam::$ACCOUNT_ID:oidc-provider/$OIDC_PROVIDER"
            },
            "Action": "sts:AssumeRoleWithWebIdentity",
            "Condition": {
                "StringEquals": {
                    "$OIDC_PROVIDER:sub": "system:serviceaccount:${EXTERNALDNS_NS:-"default"}:external-dns",
                    "$OIDC_PROVIDER:aud": "sts.amazonaws.com"
                }
            }
        }
    ]
}
EOF

IRSA_ROLE="external-dns-irsa-role"
aws iam create-role --role-name $IRSA_ROLE --assume-role-policy-document file://trust.json
aws iam attach-role-policy --role-name $IRSA_ROLE --policy-arn $POLICY_ARN

ROLE_ARN=$(aws iam get-role --role-name $IRSA_ROLE --query Role.Arn --output text)

# Create service account (skip is already created)
kubectl create serviceaccount "external-dns" --namespace ${EXTERNALDNS_NS:-"default"}

# Add annotation referencing IRSA role
kubectl patch serviceaccount "external-dns" --namespace ${EXTERNALDNS_NS:-"default"} --patch \
 "{\"metadata\": { \"annotations\": { \"eks.amazonaws.com/role-arn\": \"$ROLE_ARN\" }}}"
