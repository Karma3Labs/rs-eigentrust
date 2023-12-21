const ethers = require('ethers')
const fs = require('fs')

const walletsCount = 4
const attestationsCount = 10
const snapsCount = 4

const createDID = (address) => `did:pkh:eth:${address}`
const createSnapDID = (snapId) => `snap://${snapId}`

/*
const trustworthinessTypes = ['Quality', 'Ability', 'Flaw']
const trustworthinessScopes = {
    Quality: ['Honesty', 'Reliability'],
    Flaw: ['Dishonesty', 'Unlawful'],
    Ability: ['Software enginerring']
}
const trustworthinessLevels = ['Very low', 'Low', 'Moderate', 'High', 'Very High']
*/

const EndorsementTypes = ['EndorsementCredential', 'DisputeCredential']
const AuditReportTypes = ['AuditReportApproveCredential', 'AuditReportDisapproveCredential']
const AuditReportStatusReasons = ['Unreliable', 'Scam', 'Incomplete']
const AuditReportStatusReasonsBytes = {
    Unreliable: new Uint8Array([0x0]),
    Scam: new Uint8Array([0x1]),
    Incomplete: new Uint8Array([0x2]),
}

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

const schemaIds = {
    'EndorsementCredential': 4,
    'DisputeCredential': 4,
    'AuditReportApproveCredential': 5,
    'AuditReportDisapproveCredential': 5
}

const saveAttestationsToCSV = (attestations) => {
    const delimiter = ';'

    // https://github.com/Karma3Labs/rs-eigentrust/blob/indexer/proto-buf/services/indexer.proto#L15-L19
    const CSVData = attestations
        .map((a, i) => {
            const id = (i + 1).toString(16)
            const schema_id = schemaIds[a.type] || -1
            const schema_value = JSON.stringify(a)
            const timestamp = Date.now().toString()

            return [id, timestamp, schema_id, schema_value]
        })
        .map(row => row.join(delimiter)).join('\n')

    const timestamp = new Date().toISOString().replace(/:/g, '-').replace(/\..+/, '')
    const filename = `output-${timestamp}.csv`
    fs.writeFileSync(filename, CSVData, 'utf8')

    console.log(`${attestations.length} attestations saved to ${filename}`)
}

(async () => {
    console.log(`Generating ${walletsCount} wallets, ${attestationsCount} attestations`)

    const wallets = Array.from({ length: walletsCount }).map(() => {
        const mnemonic = ethers.Mnemonic.fromEntropy(ethers.randomBytes(32))
        const wallet = ethers.Wallet.fromPhrase(mnemonic.phrase)

        return wallet
    })

    const endorsmentAttestations = await Promise.all(
        Array.from({ length: attestationsCount }).map(async () => {
            const wallet = wallets[Math.floor(Math.random() * wallets.length)]
            const to = wallets[Math.floor(Math.random() * wallets.length)].address
            const type = EndorsementTypes[Math.floor(Math.random() * EndorsementTypes.length)]
            const attestation = await createEndorsementSchema({ wallet, to, type })

            return attestation
        }))

    const snaps = Array.from({ length: snapsCount }).map(() => {
        const snapId = ethers.keccak256(ethers.randomBytes(32)).substring(2, 10)
        return snapId
    })

    const auditReportAttestations = await Promise.all(
        Array.from({ length: attestationsCount }).map(async () => {
            const wallet = wallets[Math.floor(Math.random() * wallets.length)]
            const to = snaps[Math.floor(Math.random() * snaps.length)]
            const type = AuditReportTypes[Math.floor(Math.random() * AuditReportTypes.length)]
            const attestation = await createAuditReportSchema({ wallet, to, type })

            return attestation
        }))

    saveAttestationsToCSV([...endorsmentAttestations, ...auditReportAttestations])
})()
