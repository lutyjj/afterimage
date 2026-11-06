//! The MQTT broker's settings, which only the administrator reads and changes:
//! the deployment's, shown as they are, or else the ones saved here, which take
//! effect at once.

use afterimage_management_api::server::Extension;
use afterimage_management_api::{error, input, model, output};

use super::wire::fit;
use super::{Caller, Shared, State, internal};
use crate::telemetry::{Connection, SavedBroker};

/// The topic and discovery prefix a broker saved without them takes, as the
/// deployment's settings default to.
const TOPIC: &str = "afterimage";
const DISCOVERY_PREFIX: &str = "homeassistant";

fn administrator(caller: &Caller) -> Result<(), error::Forbidden> {
    if caller.account.administrator {
        Ok(())
    } else {
        Err(error::Forbidden {
            message: "Only the server's administrator, its first account, may do this.".to_owned(),
        })
    }
}

fn invalid(message: String) -> error::ValidationException {
    error::ValidationException {
        message,
        field_list: None,
    }
}

/// The settings in force, with the connection as it is now.
async fn current(state: &State) -> Result<model::MqttSettings, error::InternalFailure> {
    let connection = match state.telemetry.connection() {
        Connection::Off => model::MqttConnection::Off,
        Connection::Connecting => model::MqttConnection::Connecting,
        Connection::Connected => model::MqttConnection::Connected,
    };
    if let Some(deployment) = &state.deployment_broker {
        return Ok(model::MqttSettings {
            set_by_deployment: true,
            url: Some(fit(deployment.url.to_string())?),
            username: deployment.username.clone().map(fit).transpose()?,
            password_set: deployment.password.is_some(),
            topic: fit(deployment.topic.clone())?,
            discovery_prefix: fit(deployment.discovery_prefix.clone())?,
            connection,
        });
    }
    let saved = state
        .saved_broker
        .get()
        .await
        .map_err(|error| internal(&error))?;
    Ok(match saved {
        Some(saved) => model::MqttSettings {
            set_by_deployment: false,
            url: Some(fit(saved.url)?),
            username: saved.username.map(fit).transpose()?,
            password_set: saved.password.is_some(),
            topic: fit(saved.topic)?,
            discovery_prefix: fit(saved.discovery_prefix)?,
            connection,
        },
        None => model::MqttSettings {
            set_by_deployment: false,
            url: None,
            username: None,
            password_set: false,
            topic: fit(TOPIC.to_owned())?,
            discovery_prefix: fit(DISCOVERY_PREFIX.to_owned())?,
            connection,
        },
    })
}

pub async fn get_mqtt_settings(
    _input: input::GetMqttSettingsInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::GetMqttSettingsOutput, error::GetMqttSettingsError> {
    administrator(&caller)?;
    Ok(output::GetMqttSettingsOutput {
        settings: current(&state).await?,
    })
}

pub async fn update_mqtt_settings(
    input: input::UpdateMqttSettingsInput,
    Extension(state): Shared,
    caller: Caller,
) -> Result<output::UpdateMqttSettingsOutput, error::UpdateMqttSettingsError> {
    administrator(&caller)?;
    if state.deployment_broker.is_some() {
        return Err(error::Conflict {
            message: "The deployment sets the MQTT broker (AFTERIMAGE_MQTT_URL); change it there."
                .to_owned(),
        }
        .into());
    }
    let saved = state
        .saved_broker
        .get()
        .await
        .map_err(|error| internal(&error))?;
    let broker = match input.url {
        None => None,
        Some(url) => {
            let username = input.username.map(model::MqttUsername::into_inner);
            // Absent keeps the password saved; empty removes it.
            let password = match input.password.map(model::MqttPassword::into_inner) {
                None => saved.and_then(|saved| saved.password),
                Some(password) if password.is_empty() => None,
                Some(password) => Some(password),
            };
            if password.is_some() && username.is_none() {
                return Err(invalid("A password needs a username.".to_owned()).into());
            }
            let broker = SavedBroker {
                url: url.into_inner(),
                username,
                password,
                topic: input.topic.into_inner(),
                discovery_prefix: input.discovery_prefix.into_inner(),
            };
            broker
                .settings(state.public_url.clone())
                .and_then(|settings| settings.check())
                .map_err(|reason| invalid(format!("The broker's address {reason}.")))?;
            Some(broker)
        }
    };
    state
        .saved_broker
        .set(broker.as_ref(), state.clock.now())
        .await
        .map_err(|error| internal(&error))?;
    let settings = broker
        .map(|broker| broker.settings(state.public_url.clone()))
        .transpose()
        .map_err(|error| internal(&error))?;
    tracing::info!(
        broker = settings.is_some(),
        "the MQTT broker's settings changed"
    );
    state
        .telemetry
        .configure(settings)
        .await
        .map_err(|error| internal(&error))?;
    Ok(output::UpdateMqttSettingsOutput {
        settings: current(&state).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_deployments() {
        // The deployment's settings default to the same topic and prefix, so a
        // broker moved from the environment to the web UI keeps its topics.
        let config = <crate::config::Config as clap::Parser>::try_parse_from([
            "afterimage",
            "--mqtt-url",
            "mqtt://broker.example.test:1883",
        ])
        .unwrap();
        let deployment = config.mqtt().unwrap();
        assert_eq!(
            (
                deployment.topic.as_str(),
                deployment.discovery_prefix.as_str()
            ),
            (TOPIC, DISCOVERY_PREFIX)
        );
    }
}
