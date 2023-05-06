[
    {
      "name": "${ name }",
      "image": "${ ecr_repo }/${ image_name }:${ image_tag }",
      "cpu": ${ cpu },
      "memory": ${ memory },
      "portMappings": [
        {
          "containerPort": ${ container_port },
          "protocol": "tcp"
        }
      ],
      "entryPoint": [],
      "command": [],
      "environment": [
        {
          "name": "ENVIRONMENT",
          "value": "${ env }"
        },
        {
          "name": "SENTRY_ENVIRONMENT",
          "value": "${ env }"
        },
        {
          "name": "SENTRY_RELEASE",
          "value": "${ name }-${ image_tag }"
        }
      ],
      "secrets": ${ secrets_json },
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/${ env }/${ name }",
          "awslogs-region": "${ aws_region }",
          "awslogs-stream-prefix": "${ name }"
        }
      },
      "linuxParameters": {
        "initProcessEnabled": ${ init_process_enabled_json }
      }
    }
  ]
