import { useEffect, useState } from 'react'

interface AuditEntry {
  audit_id: string
  timestamp: string
  action: string
  target_type: string
  target_id: string
  result: string
  reason: string | null
}

export function AdminPage() {
  const [auditRecords, setAuditRecords] = useState<AuditEntry[]>([])
  const [billingInfo, setBillingInfo] = useState<{ total_entries: number }>({ total_entries: 0 })
  const [activeTab, setActiveTab] = useState<'audit' | 'billing'>('audit')

  useEffect(() => {
    fetch('/api/v1/audit')
      .then(r => r.json())
      .then(res => { if (res.data) setAuditRecords(res.data.items || []) })
      .catch(console.error)

    fetch('/api/v1/billing/summary')
      .then(r => r.json())
      .then(res => { if (res.data) setBillingInfo(res.data) })
      .catch(console.error)
  }, [])

  const tabs = [
    { id: 'audit' as const, label: 'Audit Log' },
    { id: 'billing' as const, label: 'Billing' },
  ]

  return (
    <div>
      <h1 style={{ fontSize: 24, fontWeight: 700, marginBottom: 24 }}>Administration</h1>

      <div style={{ display: 'flex', gap: 0, marginBottom: 24 }}>
        {tabs.map((tab) => (
          <button key={tab.id} onClick={() => setActiveTab(tab.id)} style={{
            padding: '8px 20px', fontSize: 13, border: 'none', cursor: 'pointer',
            background: activeTab === tab.id ? '#7c3aed' : '#1e1e2e',
            color: activeTab === tab.id ? '#fff' : '#a1a1aa',
            borderRadius: tab.id === 'audit' ? '6px 0 0 6px' : '0 6px 6px 0',
          }}>
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'audit' && (
        <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, overflow: 'hidden' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ borderBottom: '1px solid #27272a', color: '#71717a', fontSize: 11, textTransform: 'uppercase' as const }}>
                <th style={{ padding: '10px 12px', textAlign: 'left' }}>Time</th>
                <th style={{ padding: '10px 12px', textAlign: 'left' }}>Action</th>
                <th style={{ padding: '10px 12px', textAlign: 'left' }}>Target</th>
                <th style={{ padding: '10px 12px', textAlign: 'left' }}>Result</th>
                <th style={{ padding: '10px 12px', textAlign: 'left' }}>Reason</th>
              </tr>
            </thead>
            <tbody>
              {auditRecords.map((r, i) => (
                <tr key={i} style={{ borderBottom: '1px solid #1e1e2e' }}>
                  <td style={{ padding: '8px 12px', color: '#71717a', fontSize: 11 }}>{new Date(r.timestamp).toLocaleString()}</td>
                  <td style={{ padding: '8px 12px' }}>{r.action}</td>
                  <td style={{ padding: '8px 12px', fontFamily: 'monospace', fontSize: 11 }}>{r.target_type}/{r.target_id}</td>
                  <td style={{ padding: '8px 12px' }}>
                    <span style={{ padding: '2px 6px', borderRadius: 3, fontSize: 11, background: r.result === 'success' ? '#22c55e22' : '#ef444422', color: r.result === 'success' ? '#22c55e' : '#ef4444' }}>
                      {r.result}
                    </span>
                  </td>
                  <td style={{ padding: '8px 12px', color: '#71717a', fontSize: 12 }}>{r.reason || '-'}</td>
                </tr>
              ))}
              {auditRecords.length === 0 && (
                <tr><td colSpan={5} style={{ padding: 24, textAlign: 'center', color: '#3f3f46' }}>No audit records</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {activeTab === 'billing' && (
        <div>
          <div style={{ background: '#111118', border: '1px solid #27272a', borderRadius: 8, padding: 24, textAlign: 'center' as const }}>
            <div style={{ fontSize: 48, fontWeight: 700, color: '#a78bfa' }}>{billingInfo.total_entries}</div>
            <div style={{ fontSize: 14, color: '#71717a', marginTop: 8 }}>Total Billing Entries</div>
          </div>
        </div>
      )}
    </div>
  )
}
