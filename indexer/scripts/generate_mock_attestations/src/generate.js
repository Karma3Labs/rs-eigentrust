const ethers = require('ethers')
const {
    schemaIds,
    EndorsementTypes,
    AuditReportTypes,
} = require('./constants')
const { createEndorsementSchema } = require('./endorsementSchema')
const { createAuditReportSchema } = require('./auditReportSchema')
const { saveAttestationsToCSV } = require('./csv')


const generate = async (
    walletsCount = 4,
    snapsCount = 4,
    p2pAttestationsCount = 1,
    snapAttestationsCount = 1,
) => {
    console.log(`Generating 
    ${walletsCount} wallets, 
    ${snapsCount} snaps, 
    ${p2pAttestationsCount} p2p attestations, 
    ${snapAttestationsCount} snap attestations
    `)

    const wallets = Array.from({ length: walletsCount }).map(() => {
        const mnemonic = ethers.Mnemonic.fromEntropy(ethers.randomBytes(32))
        const wallet = ethers.Wallet.fromPhrase(mnemonic.phrase)

        return wallet
    })

    const endorsmentAttestations = await Promise.all(
        Array.from({ length: p2pAttestationsCount }).map(async () => {
            const wallet = wallets[Math.floor(Math.random() * wallets.length)]
            const to = wallets[Math.floor(Math.random() * wallets.length)].address
            const level = EndorsementTypes[Math.floor(Math.random() * EndorsementTypes.length)]
            const attestation = await createEndorsementSchema({ wallet, to, level })

            return attestation
        }))

    const snaps = Array.from({ length: snapsCount }).map(() => {
        const snapId = ethers.keccak256(ethers.randomBytes(32)).substring(0, 42)
        return snapId
    })

    const auditReportAttestations = await Promise.all(
        Array.from({ length: snapAttestationsCount }).map(async () => {
            const wallet = wallets[Math.floor(Math.random() * wallets.length)]
            const to = snaps[Math.floor(Math.random() * snaps.length)]
            const type = AuditReportTypes[Math.floor(Math.random() * AuditReportTypes.length)]
            const attestation = await createAuditReportSchema({ wallet, to, type })

            return attestation
        }))

    // console.log(JSON.stringify(endorsmentAttestations, null, '\t'))
    // console.log(JSON.stringify(auditReportAttestations, null, '\t'))
    saveAttestationsToCSV([...endorsmentAttestations, ...auditReportAttestations])
}

module.exports = {
    generate
}
