import { describe, it, expect } from 'vitest'
import { classifyRequest } from './sw-route'

describe('classifyRequest', () => {
  it('returns bypass when ?nosw=1 is present, regardless of path', () => {
    expect(classifyRequest('https://app.testudo.fyi/api/v1/journal/overview?nosw=1', 'cors')).toBe('bypass')
    expect(classifyRequest('https://app.testudo.fyi/desk/?nosw=1', 'navigate')).toBe('bypass')
    expect(classifyRequest('https://app.testudo.fyi/assets/font.woff2?nosw=1', 'no-cors')).toBe('bypass')
  })

  it('returns api for /api/* requests', () => {
    expect(classifyRequest('https://app.testudo.fyi/api/v1/journal/analytics/batch', 'cors')).toBe('api')
    expect(classifyRequest('https://app.testudo.fyi/api/health', 'no-cors')).toBe('api')
  })

  it('returns font for *.woff2 paths', () => {
    expect(classifyRequest('https://app.testudo.fyi/assets/inter-var.woff2', 'no-cors')).toBe('font')
    expect(classifyRequest('https://cdn.example.com/fonts/serif.woff2', 'cors')).toBe('font')
  })

  it('returns navigate when mode is navigate (and path is not api/font)', () => {
    expect(classifyRequest('https://app.testudo.fyi/desk/', 'navigate')).toBe('navigate')
    expect(classifyRequest('https://app.testudo.fyi/desk/trades', 'navigate')).toBe('navigate')
  })

  it('returns passthrough as the catch-all for anything else', () => {
    expect(classifyRequest('https://app.testudo.fyi/assets/index-abc.js', 'no-cors')).toBe('passthrough')
    expect(classifyRequest('https://app.testudo.fyi/assets/index-abc.css', 'no-cors')).toBe('passthrough')
    expect(classifyRequest('https://api.example.com/v1/something', 'cors')).toBe('passthrough')
  })

  it('priority: api beats font when both could match', () => {
    // Synthetic — a /api/* path that ends in .woff2 should classify as api
    expect(classifyRequest('https://app.testudo.fyi/api/proxied/font.woff2', 'no-cors')).toBe('api')
  })

  it('priority: bypass beats api', () => {
    expect(classifyRequest('https://app.testudo.fyi/api/v1/journal/overview?nosw=1', 'cors')).toBe('bypass')
  })

  it('returns passthrough for unparseable URLs', () => {
    // `new URL('::garbage::', 'http://localhost')` throws — handled defensively
    expect(classifyRequest('::garbage::', 'cors')).toBe('passthrough')
  })
})
