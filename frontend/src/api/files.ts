import client from './client'

export interface FileEntry {
  name: string
  size: number
  is_dir: boolean
}

export function listFiles(path?: string) {
  return client.get<FileEntry[]>('/files', { params: path ? { path } : {} })
}

export function getDownloadUrl(filePath: string): string {
  const auth = JSON.parse(localStorage.getItem('auth') || '{}')
  const token = auth.token || ''
  // 用 URL 传 token，因为 <a> 标签无法带 Authorization header
  return `/api/download?path=${encodeURIComponent(filePath)}&token=${encodeURIComponent(token)}`
}
