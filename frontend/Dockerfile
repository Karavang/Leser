# Сборка отдельно от рантайма: в финальный образ едет только dist, без node.
FROM node:22-alpine AS build
WORKDIR /app

# Зависимости отдельным слоем: правка src не переустанавливает пакеты.
COPY package.json package-lock.json ./
RUN npm ci

COPY . .
RUN npm run build

FROM nginx:1.27-alpine
COPY nginx.docker.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
