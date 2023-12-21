const ethers = require('ethers')
const fs = require('fs')
const {
    schemaIds,
    EndorsementTypes,
    AuditReportTypes,
} = require('./constants')
const { createEndorsementSchema } = require('./endorsementSchema')
const { createAuditReportSchema } = require('./auditReportSchema')

const walletsCount = 4
const attestationsCount = 10
const snapsCount = 4

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
        const snapId = ethers.keccak256(ethers.randomBytes(32)).substring(0, 12)
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
