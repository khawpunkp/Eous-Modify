import { createApp } from 'vue';
import { createPinia } from 'pinia';
import { createRouter, createWebHistory } from 'vue-router';
import { routes } from 'vue-router/auto-routes';
import { autoAnimatePlugin } from '@formkit/auto-animate/vue';

import App from './App.vue';
import './styles/main.css';

const router = createRouter({
   history: createWebHistory(),
   routes: [...routes, { path: '/', redirect: '/agents' }],
});

const app = createApp(App);
app.use(createPinia());
app.use(router);
app.use(autoAnimatePlugin);
app.mount('#app');
