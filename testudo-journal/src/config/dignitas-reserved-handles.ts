/** @anchor infra:journal-config:dignitas-reserved-handles
 * @tags infra */

// UX-only pre-submit hint — backend is the enforcement gate
export const RESERVED_HANDLES = new Set([
  'admin', 'testudo', 'api', 'www', 'root', 'support', 'help',
  'mod', 'team', 'official', 'cz', 'sbf', 'vitalik', 'staff',
  'security', 'billing', 'legal', 'abuse', 'postmaster', 'hostmaster',
])
