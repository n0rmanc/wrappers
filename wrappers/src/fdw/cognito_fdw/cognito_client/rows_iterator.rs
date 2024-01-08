use crate::fdw::cognito_fdw::cognito_client::row::IntoRow;
use crate::fdw::cognito_fdw::cognito_client::CognitoClientError;

use crate::fdw::cognito_fdw::cognito_client::CreateRuntimeError;

use std::collections::VecDeque;
use supabase_wrappers::prelude::{Column, Row};

pub(crate) struct RowsIterator {
    cognito_client: aws_sdk_cognitoidentityprovider::Client,
    columns: Vec<Column>,
    rows: VecDeque<Row>,
    have_more_rows: bool,
    pagination_token: Option<String>,
}

impl RowsIterator {
    pub(crate) fn new(
        columns: Vec<Column>,
        cognito_client: aws_sdk_cognitoidentityprovider::Client,
    ) -> Self {
        Self {
            columns,
            cognito_client,
            rows: VecDeque::new(),
            have_more_rows: true,
            pagination_token: None,
        }
    }

    fn fetch_rows_batch(&mut self) -> Result<Option<Row>, CognitoClientError> {
        self.have_more_rows = false;
        let rt = tokio::runtime::Runtime::new()
            .map_err(CreateRuntimeError::FailedToCreateAsyncRuntime)?;

        let mut request = self
            .cognito_client
            .list_users()
            .user_pool_id("ap-southeast-2_xuUGae0Bl".to_string());

        if let Some(ref token) = self.pagination_token {
            request = request.pagination_token(token.clone());
        }
        self.rows = rt.block_on(async {
            match request.send().await {
                Ok(response) => {
                    self.pagination_token = response.pagination_token.clone();
                    response
                        .users
                        .clone()
                        .unwrap_or_else(Vec::new)
                        .into_iter()
                        .map(|u| u.into_row(&self.columns))
                        .collect::<VecDeque<Row>>()
                }
                Err(_) => {
                    VecDeque::new() // or handle the error as required
                }
            }
        });

        self.have_more_rows = self.pagination_token.is_some();
        Ok(self.get_next_row())
    }

    fn get_next_row(&mut self) -> Option<Row> {
        self.rows.pop_front()
    }
}

impl Iterator for RowsIterator {
    type Item = Result<Row, CognitoClientError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(row) = self.get_next_row() {
            Some(Ok(row))
        } else {
            if self.have_more_rows {
                self.fetch_rows_batch().transpose()
            } else {
                None
            }
        }
    }
}
