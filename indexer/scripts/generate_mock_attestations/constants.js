const schemaIds = {
    'EndorsementCredential': 4,
    'DisputeCredential': 4,
    'AuditReportApproveCredential': 5,
    'AuditReportDisapproveCredential': 5
}

const EndorsementTypes = ['EndorsementCredential', 'DisputeCredential']

const AuditReportTypes = ['AuditReportApproveCredential', 'AuditReportDisapproveCredential']

const AuditReportStatusReasons = ['Unreliable', 'Scam', 'Incomplete']

const AuditReportStatusReasonsBytes = {
    Unreliable: new Uint8Array([0x0]),
    Scam: new Uint8Array([0x1]),
    Incomplete: new Uint8Array([0x2]),
}

module.exports = {
    schemaIds,
    EndorsementTypes,
    AuditReportTypes,
    AuditReportStatusReasons,
    AuditReportStatusReasonsBytes
}