import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import 'element-plus/theme-chalk/dark/css-vars.css'
import App from './App.vue'
import './styles/theme.css'
// 引入即应用主题（持久化 + 首次跟随系统），并挂载 html.dark 类
import './theme'

const app = createApp(App)
app.use(createPinia())
app.use(ElementPlus)
app.mount('#app')
