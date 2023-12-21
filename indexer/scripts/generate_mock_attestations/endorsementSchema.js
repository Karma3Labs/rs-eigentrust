const ethers = require('ethers')
const {
    createDID,
} = require('./utils')
const {
    EndorsementTypes,
} = require('./constants')

const createEndorsementSchema = async ({
    wallet,
    to,
    type,
}) => {
    const issuer = createDID(wallet.address)
    const toDID = createDID(to)

    const attestationDetails = type === 'DisputeCredential'
        ? {
            currentStatus: "Disputed",
            statusReason: "None"
        }
        : { currentStatus: "Endorsed" }

    const schemaPayload = {
        type,
        issuer,
        credentialSubject: {
            id: toDID,
            ...attestationDetails
        },
    }

    const DIDPrefixBytes = new Uint8Array([0x00]) // stands for pkh:eth
    const issuerBytes = ethers.getBytes(wallet.address)
    const currentStatusBytes = attestationDetails.currentStatus === 'Endorsed'
        ? new Uint8Array([0x01])
        : new Uint8Array([0x00])

    const keccak256Hash = ethers.keccak256(
        ethers.concat([
            DIDPrefixBytes,
            issuerBytes,
            currentStatusBytes
        ])
    )

    const signature = await wallet.signMessage(keccak256Hash)

    const schema = {
        ...schemaPayload,
        proof: { signature }
    }

    return schema
}

module.exports = {
    createEndorsementSchema
}