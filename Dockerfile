FROM node:22-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM python:3.13-slim
WORKDIR /app
RUN pip install --no-cache-dir uv
COPY pyproject.toml uv.lock ./
RUN uv sync --frozen --no-dev
COPY backend/ backend/
COPY --from=frontend /app/frontend/dist frontend/dist
ENV QUARK_DATA_DIR=/data
EXPOSE 8000
CMD ["uv", "run", "--no-dev", "uvicorn", "backend.app:app", "--host", "0.0.0.0", "--port", "8000"]
