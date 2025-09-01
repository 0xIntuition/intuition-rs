FROM node:20-alpine

WORKDIR /app

COPY package*.json ./
RUN npm i

COPY . .

EXPOSE 4983

CMD ["npx", "drizzle-kit", "studio", "--host", "0.0.0.0", "--port", "4983"]
