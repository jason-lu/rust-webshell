import client from './client'

export interface LoginResponse {
  token: string
  username: string
}

export function login(username: string, password: string) {
  return client.post<LoginResponse>('/login', { username, password })
}

export function changePassword(username: string, old_password: string, new_password: string) {
  return client.post('/change-password', { username, old_password, new_password })
}
