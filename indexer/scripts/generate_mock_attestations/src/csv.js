const fs = require('fs')
const path = require('path')
const {
    schemaIds,
} = require('./constants')

const dir = __dirname.split('/src')[0] + '/output'

if (!fs.existsSync(dir)){
    fs.mkdirSync(dir, { recursive: true });
}

const saveAttestationsToCSV = (attestations) => {
    const delimiter = ';'

    // https://github.com/Karma3Labs/rs-eigentrust/blob/indexer/proto-buf/services/indexer.proto#L15-L19
    const CSVData = attestations
        .map((a, i) => {
            const id = (i + 1).toString(16)
            const schema_id = schemaIds[a.type] || 0
            const schema_value = JSON.stringify(a)
            const timestamp = Date.now().toString()

            return [id, timestamp, schema_id, schema_value]
        })
        .map(row => row.join(delimiter)).join('\n')

    const timestamp = new Date().toISOString().replace(/:/g, '-').replace(/\..+/, '')
    const filename = `/output/output-${timestamp}.csv`

    const filePath = path.join(process.cwd(), filename)
    fs.writeFileSync(filePath, CSVData, 'utf8')

    console.log(`${attestations.length} attestations saved to ${filename}`)
}

module.exports = {
    saveAttestationsToCSV
}