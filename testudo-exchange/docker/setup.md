Commands -

0. Create images for all services

1. Before signin in to docker - follow this - ignore if already created
   https://docs.docker.com/desktop/setup/sign-in/

2. docker login

3. Tag all images -

docker tag docker-ws-stream jogeshwar01/exchange-ws-stream:latest

4. Push all images -

docker push jogeshwar01/exchange-ws-stream:latest
