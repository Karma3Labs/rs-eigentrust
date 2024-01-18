const schemaIds = {
    'AuditReportApproveCredential': 2,
    'AuditReportDisapproveCredential': 3,
    'EndorsementCredential': 4,
    'DisputeCredential': 4,
}

const EndorsementTypes = [1, -1] // 0 - undo

const AuditReportTypes = ['AuditReportApproveCredential', 'AuditReportDisapproveCredential']

const AuditReportStatusReasons = [{
    "type": "Scam",
    "value": "Interact with a fraudulent smart contract",
    "lang": "en"
}]

const AuditReportStatusReasonsBytes = {
    Unreliable: new Uint8Array([0x0]),
    Scam: new Uint8Array([0x1]),
    Incomplete: new Uint8Array([0x2]),
}

/*
const trustworthinessTypes = ['Quality', 'Ability', 'Flaw']
const trustworthinessScopes = {
    Quality: ['Honesty', 'Reliability'],
    Flaw: ['Dishonesty', 'Unlawful'],
    Ability: ['Software enginerring']
}
const trustworthinessLevels = ['Very low', 'Low', 'Moderate', 'High', 'Very High']
*/

module.exports = {
    schemaIds,
    EndorsementTypes,
    AuditReportTypes,
    AuditReportStatusReasons,
    AuditReportStatusReasonsBytes
}