FROM ruby:alpine3.20
RUN apk add --no-cache rpm
RUN gem install fpm
WORKDIR /src
ENTRYPOINT ["/usr/local/bundle/bin/fpm"]