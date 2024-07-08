# Users

## Context

A new user needs to access demeter platform.

## Decision

The oauth platform will handle the authentication.

- Creation  
  The user needs to log in using the oauth. The oauth token will be sent to the user creation RPC and the user domain will validate the user data, integrating with oauth to get the user profile data and send an event to the queue creating the account.

- Invite  
  When a user invites another user to a `project`, a temporary token is created and sent to the new user's email. The new user will click on the link and if there's not an account, the user domain will create the account following the user creation flow. Then, an event is sent to link the new user to that project.

## Rules

- Users creation
- Users invitation to a project
  - Invitation needs to be accepted
  - Generation random token
  - Integrate with AWS to send the email(Email driven)
  - Send an invitation event
  - Event driver will save the cache invitation event
- Users accept invite
  - Validate the token getting from the cache
  - Validate if the user exists
  - Send an event to link the user to the project
- Users deletion from a project
