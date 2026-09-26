import { ref, computed } from 'vue'
import { fetchRuleProviders, fetchRules } from '@/api'
import type { Rule, RuleProvider } from '@/types'

const rules = ref<Rule[]>([])
const loading = ref(false)
const filterText = ref('')
const ruleProviders = ref<RuleProvider[]>([])
const providersAvailable = computed(() => ruleProviders.value.length > 0)

let rulesRequest = 0
let providersRequest = 0

const filteredRules = computed(() => {
  if (!filterText.value) return rules.value
  const q = filterText.value.toLowerCase()
  return rules.value.filter(
    (r) =>
      r.payload.toLowerCase().includes(q) ||
      r.proxy.toLowerCase().includes(q) ||
      r.type.toLowerCase().includes(q),
  )
})

export function useRulesStore() {
  async function loadRules() {
    const request = ++rulesRequest
    loading.value = true
    try {
      const { data } = await fetchRules()
      if (request === rulesRequest) rules.value = data.rules
    } catch {
      if (request === rulesRequest) rules.value = []
    } finally {
      if (request === rulesRequest) loading.value = false
    }
  }

  async function loadProviders() {
    const request = ++providersRequest
    try {
      const { data } = await fetchRuleProviders()
      if (request === providersRequest) ruleProviders.value = Object.values(data?.providers ?? {})
    } catch {
      if (request === providersRequest) ruleProviders.value = []
    }
  }

  async function reloadAfterCoreReady() {
    await Promise.all([loadRules(), loadProviders()])
  }

  return {
    rules,
    filteredRules,
    loading,
    filterText,
    ruleProviders,
    providersAvailable,
    loadRules,
    loadProviders,
    reloadAfterCoreReady,
  }
}
