const createDID = (address) => `did:pkh:eth:${address}`
const createSnapDID = (snapId) => `snap://${snapId}`

module.exports = {
    createDID,
    createSnapDID
}