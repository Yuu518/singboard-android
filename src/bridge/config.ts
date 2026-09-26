import { invoke } from '@tauri-apps/api/core'

export interface DetectedRuntimeFiles {
  baseDir: string
  singboxPath?: string
  coreInstallPath: string
  configPath?: string
  rulesDir?: string
  found: boolean
}

export async function validateSingboxConfig(
  singboxPath: string,
  configPath: string,
  workingDir?: string,
): Promise<string> {
  return invoke<string>('validate_config', { singboxPath, configPath, workingDir })
}

export async function getSingboxVersion(singboxPath: string): Promise<string> {
  return invoke<string>('get_singbox_version', { singboxPath })
}

export async function getFileHash(path: string): Promise<string> {
  return invoke<string>('get_file_hash', { path })
}

export async function detectRuntimeFiles(baseDir: string): Promise<DetectedRuntimeFiles> {
  return invoke<DetectedRuntimeFiles>('detect_runtime_files', { baseDir })
}

export async function srsListProvider(
  workingDir: string,
  configPath: string,
  singboxPath: string,
  tag: string,
): Promise<Array<{ type: string; value: string }>> {
  return invoke<Array<{ type: string; value: string }>>('srs_list_provider', { workingDir, configPath, singboxPath, tag })
}

export async function srsMatchProvider(
  workingDir: string,
  configPath: string,
  singboxPath: string,
  tag: string,
  query: string,
): Promise<boolean> {
  return invoke<boolean>('srs_match_provider', { workingDir, configPath, singboxPath, tag, query })
}
