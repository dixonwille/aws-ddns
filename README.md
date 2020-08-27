# AWS DDNS

Using AWS API Gateway and Lambda functions, create a DDNS provider for personal use.

This application is very minimum! It only has two endpoints. One is to create users to be authenticated with and the other is the actual endpoint routers can hit.

## Tools needed

- https://www.serverless.com/framework/docs/getting-started/
- https://www.serverless.com/plugins/serverless-rust
    - can use `sls plugin install serverless-rust`
- https://www.rust-lang.org/tools/install
    - will also need to run `rustup target add x86_64-unknown-linux-musl` so it can install on local instead of docker

## Deploy

To deploy this serverless application you will need to setup AWS Credentials (https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-files.html)

After that is setup you can install as many stages as you want.

The following will deploy to the `dev` stage. This stage is useful for testing out changes.


```sh
sls deploy --conceal #hide the secret api key from output
```

The following will deploy to the `prod` stage. I use this stage for the final API that is used by my router.

```sh
sls deploy -s prod --conceal #hide the secret api key from output
```

## Getting the settings

After everything is deployed, you can go through the AWS console and setup custom domains and retrieve the API key used for the Create User endpoint.

## Usage

### POST /user

Creates a user that can be used to authenticate with.

You will need the following headers set:

- `Content-Type: application/json`
- `x-api-key: <KEY>` - The key can be found in the API Gateway portion of AWS console. This is the Admin key and with it, users can be created. So don't share it.

The following is an example body:

- `username` - cannot contain a colon (:) and be greater than 7 characters
- `password` - greater than 7 characters

```json
{
    "username": "someuser",
    "password": "awesomePass",
    "domains": [
        "home.domain.com"
    ]
}
```

### GET /nic/update

This endpoint is what routers should hit. It is roughtly based on this https://help.dyn.com/remote-access-api/perform-update/. I do not follow it completley but this could be made to follow it more closely if desired.

> For my case I did not need to follow it completley so only used it as a guide

This endpoint requires just the `Authorization` header with a value in `Basic` auth format.

Query parameters are needed

- `hostname` - comma seperated list of hostnames to update
    - you can supply multiple hostname parameters instead
    - cannot have a duplicate entry
- `myip` - expects to be the IPv4 to update the record to
