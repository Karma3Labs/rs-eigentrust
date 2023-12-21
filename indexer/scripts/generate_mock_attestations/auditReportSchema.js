const ethers = require('ethers')
const {
    createDID,
    createSnapDID
} = require('./utils')
const {
    AuditReportTypes,
    AuditReportStatusReasons,
    AuditReportStatusReasonsBytes
} = require('./constants')


// https://hackmd.io/@VT6Lc8FNQL2AllbBc32ERg/H1akxxBrT
const createAuditReportSchema = async ({
    wallet,
    to,
    type,
}) => {
    const issuer = createDID(wallet.address)
    const toDID = createSnapDID(to)

    const statusReason = AuditReportStatusReasons[Math.floor(Math.random() * AuditReportStatusReasons.length)]

    const attestationDetails = type === 'AuditReportDisapproveCredential'
        ? {
            statusReason
        }
        : {}

    const schemaPayload = {
        type,
        issuer,
        credentialSubject: {
            id: toDID,
            ...attestationDetails
        },
    }

    const utf8Buffer = Buffer.from(to, 'utf-8');
    const snapIdBytes = new Uint8Array(utf8Buffer)

    const currentStatusBytes = attestationDetails.currentStatus === 'AuditReportDisapproveCredential'
        ? AuditReportStatusReasonsBytes[currentStatus]
        : new Uint8Array([])

    const keccak256Hash = ethers.keccak256(
        ethers.concat([
            snapIdBytes,
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
    createAuditReportSchema
}