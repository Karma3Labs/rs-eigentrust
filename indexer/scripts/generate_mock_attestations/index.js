const ethers = require('ethers')
const fs = require('fs')

const walletsCount = 1000
const attestationsCount = 1000

const createDID = (address) => `did:pkh:eth:${address}`

/*
const trustworthinessTypes = ['Quality', 'Ability', 'Flaw']
const trustworthinessScopes = {
    Quality: ['Honesty', 'Reliability'],
    Flaw: ['Dishonesty', 'Unlawful'],
    Ability: ['Software enginerring']
}
const trustworthinessLevels = ['Very low', 'Low', 'Moderate', 'High', 'Very High']
*/

const credentialTypes = ['EndorsementCredential', 'DisputeCredential']

// https://hackmd.io/@VT6Lc8FNQL2AllbBc32ERg/H1akxxBrT
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

const saveAttestationsToCSV = (filename, attestations) => {
    const delimiter = ';'

    // https://github.com/Karma3Labs/rs-eigentrust/blob/indexer/proto-buf/services/indexer.proto#L15-L19
    const CSVData = attestations
        .map((a, i) => {
            const id = (i + 1).toString(16)
            const schema_id = '1'
            const schema_value = JSON.stringify(a)
            const timestamp = Date.now().toString()

            return [id, timestamp, schema_id, schema_value]
        })
        .map(row => row.join(delimiter)).join('\n')

    fs.writeFileSync(filename, CSVData, 'utf8')
}

(async () => {
    console.log(`Generating ${walletsCount} wallets, ${attestationsCount} attestations`)

    const wallets = Array.from({ length: walletsCount }).map(() => {
        const mnemonic = ethers.Mnemonic.fromEntropy(ethers.randomBytes(32))
        const wallet = ethers.Wallet.fromPhrase(mnemonic.phrase)

        return wallet
    })

    const attestations = await Promise.all(
        Array.from({ length: attestationsCount }).map(async () => {
            const wallet = wallets[Math.floor(Math.random() * wallets.length)]
            const to = wallets[Math.floor(Math.random() * wallets.length)].address
            const type = credentialTypes[Math.floor(Math.random() * credentialTypes.length)]
            const attestation = await createEndorsementSchema({ wallet, to, type })

            return attestation
        }))

    const filename = 'output.csv'
    saveAttestationsToCSV(filename, attestations)

    console.log(`${attestations.length} attestations saved to ${filename}`)
})()
