/** @anchor api:journal:journal
 * @tags api */

import { fetchCrud, fetchWithCredentials, API_BASE } from './core'
import type { JournalTag, JournalEntry, SetupTagEntry } from './types'

export async function fetchTags(): Promise<JournalTag[]> {
    return fetchCrud<JournalTag[]>('tags')
}

export async function fetchUserSetupTags(limit = 20): Promise<SetupTagEntry[]> {
    return fetchCrud<SetupTagEntry[]>(`setup-tags?limit=${limit}`)
}

export async function fetchEntries(params: {
    tradeId?: string; page?: number; limit?: number
}): Promise<{ entries: JournalEntry[]; total: number }> {
    const p = new URLSearchParams()
    if (params.tradeId) p.set('trade_id', params.tradeId)
    if (params.page) p.set('page', String(params.page))
    if (params.limit) p.set('limit', String(params.limit))
    return fetchCrud(`entries?${p}`)
}

export async function createEntry(data: {
    title: string; body: string; entry_type?: string
    trade_id?: string; entry_date?: string
}): Promise<JournalEntry> {
    return fetchCrud<JournalEntry>('entries', {
        method: 'POST', body: JSON.stringify(data),
    })
}

export async function updateEntry(entryId: string, data: {
    title: string; body: string; entry_type?: string
}): Promise<JournalEntry> {
    return fetchCrud<JournalEntry>(`entries/${entryId}`, {
        method: 'PUT', body: JSON.stringify(data),
    })
}

export async function deleteEntry(entryId: string): Promise<void> {
    await fetchCrud<{ deleted: boolean }>(`entries/${entryId}`, { method: 'DELETE' })
}

export async function createTag(data: {
    name: string; color?: string
}): Promise<JournalTag> {
    return fetchCrud<JournalTag>('tags', {
        method: 'POST', body: JSON.stringify(data),
    })
}

export async function updateTag(tagId: string, data: {
    name?: string; color?: string
}): Promise<JournalTag> {
    return fetchCrud<JournalTag>(`tags/${tagId}`, {
        method: 'PUT', body: JSON.stringify(data),
    })
}

export async function deleteTag(tagId: string): Promise<void> {
    await fetchCrud<{ deleted: boolean }>(`tags/${tagId}`, { method: 'DELETE' })
}

// ─── Image upload + storage ───

export class UploadError extends Error {
    constructor(
        message: string,
        public code?: string,
        public details?: { used_bytes?: number; quota_bytes?: number; remaining_bytes?: number },
    ) {
        super(message)
        this.name = 'UploadError'
    }
}

export interface StorageUsage {
    used_bytes: number; quota_bytes: number; image_count: number
}

export async function uploadJournalImage(file: File): Promise<{ url: string }> {
    const formData = new FormData()
    formData.append('file', file)
    const res = await fetchWithCredentials(`${API_BASE}/api/v1/journal/upload`, {
        method: 'POST', body: formData,
    })
    if (!res.ok) {
        const err = await res.json().catch(() => ({ message: `Upload failed: ${res.status}` }))
        throw new UploadError(err.message || `Upload failed: ${res.status}`, err.error, err.details)
    }
    return res.json()
}

export async function fetchStorageUsage(): Promise<StorageUsage> {
    return fetchCrud<StorageUsage>('storage')
}

export async function deleteImage(imageId: string): Promise<void> {
    await fetchCrud<{ deleted: boolean }>(`images/${imageId}`, { method: 'DELETE' })
}
