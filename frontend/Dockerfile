# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS base
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm install --frozen-lockfile
COPY . .

FROM base AS build
RUN pnpm run build

# Static build served by nginx. /api is proxied to API_URL, mirroring the
# Vite dev proxy.
FROM nginx:1.29-alpine AS prod
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /app/dist /usr/share/nginx/html

FROM base AS dev
# Default only for documentation; actual port comes from FRONTEND_PORT
# (read in vite.config.ts) and is published via docker-compose.
EXPOSE 5173
CMD ["pnpm", "exec", "vite"]
