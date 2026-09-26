import { ref } from 'vue'

export const appVisible = ref(document.visibilityState !== 'hidden')

document.addEventListener('visibilitychange', () => {
  appVisible.value = document.visibilityState !== 'hidden'
})
