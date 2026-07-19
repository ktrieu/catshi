# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim
ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable

WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,id=pnpm,target=/pnpm/store pnpm install --frozen-lockfile
COPY . .

# Default only for documentation; actual port comes from FRONTEND_PORT
# (read in vite.config.ts) and is published via docker-compose.
EXPOSE 5173
CMD ["pnpm", "exec", "vite"]
