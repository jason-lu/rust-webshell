import client from './client'

export function uploadFile(file: File) {
  const form = new FormData()
  form.append('file', file)
  return client.post('/upload', form)
}
