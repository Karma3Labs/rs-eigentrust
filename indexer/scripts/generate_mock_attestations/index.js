const ethers = require('ethers')
const fs = require('fs')

const walletsCount = 10
const attestationsCount = 10

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
            statusReason: {
                "value": "Suspicious activities",
                "lang": "en"
            }
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


    const schemaPayloadSerialized = JSON.stringify(schemaPayload)
    const hash = ethers.hashMessage(schemaPayloadSerialized)
    const signature = await wallet.signMessage(hash)

    const schema = {
        ...schemaPayload,
        proof: { signature }
    }

    return schema
}

const saveAttestationsToCSV = attestations => {
    const filename = 'output.csv'

    const csvData = attestations
        .map((a, i) => {
            const id = (i + 1).toString(16)
            const schema_id = '0x1'
            const schema_value = JSON.stringify(a)
            const timestamp = Date.now().toString()

            return [id, timestamp, schema_id, schema_value]
        })
        .map(row => row.join(',')).join('\n')

    fs.writeFileSync(filename, csvData, 'utf8')
    console.log(`${attestations.length} results saved to ${filename}`)
}

(async () => {
    const wallets = Array.from({ length: walletsCount }).map(() => {
        const mnemonic = ethers.Mnemonic.fromEntropy(ethers.randomBytes(32))
        const wallet = ethers.Wallet.fromPhrase(mnemonic.phrase)
        return wallet
    })

    const attestations = await Promise.all(Array.from({ length: attestationsCount }).map(async () => {
        const wallet = wallets[Math.floor(Math.random() * wallets.length)]
        const to = wallets[Math.floor(Math.random() * wallets.length)].address
        const type = credentialTypes[Math.floor(Math.random() * credentialTypes.length)]
        const attestation = await createEndorsementSchema({ wallet, to, type })

        return attestation
    }))

    saveAttestationsToCSV(attestations)
})()
