use crate::core::wallet::WalletInfo;
use iced::widget::{
    button, column, container, horizontal_rule, row, scrollable, text, text_input,
};
use iced::{Alignment, Element, Length, Padding};

#[derive(Debug, Clone)]
pub enum Message {
    GenerateSingle,
    StartIndefinite,
    StopIndefinite,
    CancelSingle,
    WalletChecked(WalletInfo),
    ErrorOccurred(String),
    ToggleWordCount,
    ApiUrlChanged(String),
    FallbackApiUrlChanged(String),
    Tick,
    CopyToClipboard(String),
}

// Re-export UiState for convenience
pub use super::ui_state::UiState;

impl UiState {
    /// Helper to create a copy button
    fn copy_button(text_to_copy: String) -> button::Button<'static, Message> {
        button("📋")
            .padding(5)
            .on_press(Message::CopyToClipboard(text_to_copy))
    }

    /// Helper to create masked text element
    fn masked_text(message: &str) -> Element<'_, Message> {
        Element::from(text(message))
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = column![
            // Header
            self.create_header(),
            horizontal_rule(1),
            
            // Stats Panel
            self.create_stats_panel(),
            
            // Control Panel
            self.create_control_panel(),
            
            // Error Display
            self.create_error_display(),
            
            // Main Content Area
            self.create_main_content(),
        ]
        .spacing(10)
        .padding(20)
        .width(Length::Fill)
        .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn create_header(&self) -> Element<'_, Message> {
        container(
            text("🔐 Mnemonic Wallet Guesser").size(32),
        )
        .width(Length::Fill)
        .padding(Padding::from([10, 0]))
        .center_x()
        .into()
    }

    fn create_stats_panel(&self) -> Element<'_, Message> {
        let mut stats_content = column![
            text("📊 Statistics").size(20),
            horizontal_rule(1),
            row![
                text("Attempts: ").size(18),
                text(format!("{}", self.attempts))
                    .size(20)
                    .width(Length::Shrink),
            ]
            .spacing(10)
            .align_items(Alignment::Center),
        ]
        .spacing(8);

        if let Some(ref wallet) = self.current_wallet {
            stats_content = stats_content.push(
                row![
                    text("Current Balance: ").size(16),
                    text(format!("{} sat", wallet.total_balance)).size(16),
                ]
                .spacing(10),
            );
        }

        if self.is_guessing {
            stats_content = stats_content.push(
                row![
                    text("🔄").size(18),
                    text("Guessing indefinitely...").size(16),
                ]
                .spacing(10)
                .align_items(Alignment::Center),
            );
        }

        container(stats_content)
            .padding(15)
            .width(Length::Fill)
            .style(iced::theme::Container::Box)
            .into()
    }

    fn create_control_panel(&self) -> Element<'_, Message> {
        container(
            column![
                text("⚙️ Controls").size(18),
                horizontal_rule(1),
                row![
                    if self.is_generating_single {
                        button("⏹ Stop Generating")
                            .padding([10, 15])
                            .width(Length::Fixed(150.0))
                            .on_press(Message::CancelSingle)
                    } else {
                        button("🎲 Generate Single")
                            .padding([10, 15])
                            .width(Length::Fixed(150.0))
                            .on_press(Message::GenerateSingle)
                    },
                    if self.is_guessing {
                        button("⏹ Stop Guessing")
                            .padding([10, 15])
                            .width(Length::Fixed(180.0))
                            .on_press(Message::StopIndefinite)
                    } else {
                        button("🚀 Start Indefinite Guessing")
                            .padding([10, 15])
                            .width(Length::Fixed(180.0))
                            .on_press(Message::StartIndefinite)
                    },
                ]
                .spacing(15)
                .align_items(Alignment::Center),
                text("Configuration").size(16),
                row![
                    text("API URL:").width(Length::Fixed(100.0)),
                    text_input("Enter API URL", &self.api_url_input)
                        .width(Length::Fill)
                        .on_input(Message::ApiUrlChanged),
                ]
                .spacing(10)
                .align_items(Alignment::Center),
                row![
                    text("Fallback API:").width(Length::Fixed(100.0)),
                    text_input("Leave blank for none", &self.fallback_api_url_input)
                        .width(Length::Fill)
                        .on_input(Message::FallbackApiUrlChanged),
                ]
                .spacing(10)
                .align_items(Alignment::Center),
                row![
                    text(format!("Word Count: {} words", self.config.wallet.word_count)),
                    button("Toggle 12/24 Words")
                        .padding(8)
                        .on_press(Message::ToggleWordCount),
                ]
                .spacing(15)
                .align_items(Alignment::Center),
            ]
            .spacing(10),
        )
        .padding(15)
        .width(Length::Fill)
        .into()
    }

    fn create_error_display(&self) -> Element<'_, Message> {
        if let Some(ref error) = self.last_error {
            container(
                column![
                    text("❌ Error:").size(16),
                    text(error).size(14),
                ]
                .spacing(5),
            )
            .padding(15)
            .width(Length::Fill)
            .style(iced::theme::Container::Box)
            .into()
        } else {
            container(text("")).height(Length::Fixed(1.0)).into()
        }
    }

    fn create_main_content(&self) -> Element<'_, Message> {
        row![
            // Current Wallet
            container(
                column![
                    text("Current Wallet").size(18),
                    horizontal_rule(1),
                    {
                        if let Some(ref wallet) = self.current_wallet {
                            Element::from(
                                scrollable(self.create_wallet_details(wallet))
                                    .height(Length::Fill)
                            )
                        } else {
                            Element::from(
                                container(text("No wallet generated yet"))
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                                    .center_x()
                                    .center_y()
                            )
                        }
                    },
                ]
                .spacing(10),
            )
            .padding(15)
            .width(Length::FillPortion(1))
            .height(Length::Fill)
            .style(iced::theme::Container::Box),
            
            // Found Wallet
            container(
                column![
                    text("🎉 Found Wallet (With Balance!)")
                        .size(18),
                    horizontal_rule(1),
                    {
                        if let Some(ref wallet) = self.found_wallet {
                            Element::from(
                                scrollable(self.create_wallet_details(wallet))
                                    .height(Length::Fill)
                            )
                        } else {
                            Element::from(
                                container(text("")).width(Length::Fill).height(Length::Fill)
                            )
                        }
                    },
                ]
                .spacing(10),
            )
            .padding(15)
            .width(Length::FillPortion(1))
            .height(Length::Fill)
            .style(iced::theme::Container::Box),
        ]
        .spacing(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn create_wallet_details(&self, wallet: &WalletInfo) -> Element<'_, Message> {
        let mut content = column![
            // Wallet Summary
            text("Wallet Summary").size(16),
            horizontal_rule(1),
            
            // Mnemonic
            row![
                text("Mnemonic:").width(Length::Fixed(120.0)),
                if wallet.has_balance() {
                    Element::from(
                        row![
                            text(&wallet.mnemonic).size(12),
                            Self::copy_button(wallet.mnemonic.clone())
                                .padding([5, 10]),
                        ]
                        .spacing(10)
                    )
                } else {
                    Self::masked_text("*** MASKED - No balance found ***")
                },
            ]
            .spacing(10)
            .align_items(Alignment::Center),
            
            row![
                text("Word Count:").width(Length::Fixed(120.0)),
                text(format!("{} words", wallet.word_count)),
            ]
            .spacing(10),
            
            row![
                text("Total Balance:").width(Length::Fixed(120.0)),
                                text(format!("{} sat", wallet.total_balance)),
            ]
            .spacing(10),
            
            text("Addresses").size(16),
            horizontal_rule(1),
        ]
        .spacing(8);

        // Addresses
        for addr_info in &wallet.addresses {
            content = content.push(
                    container(
                        column![
                            row![
                                text(&addr_info.address_type).size(14),
                                text(&addr_info.derivation_path).size(12),
                            ]
                            .spacing(10),
                            row![
                                text("Address:").width(Length::Fixed(120.0)),
                                row![
                                    text(&addr_info.address).size(11),
                                    Self::copy_button(addr_info.address.clone()),
                                ]
                                .spacing(10)
                                .align_items(Alignment::Center),
                            ]
                            .spacing(10),
                            row![
                                text("Private Key:").width(Length::Fixed(120.0)),
                                if addr_info.has_balance() {
                                    Element::from(
                                        row![
                                            text(&addr_info.private_key).size(11),
                                            Self::copy_button(addr_info.private_key.clone()),
                                        ]
                                        .spacing(10)
                                        .align_items(Alignment::Center)
                                    )
                                } else {
                                    Self::masked_text("*** MASKED - No balance ***")
                                },
                            ]
                            .spacing(10),
                            row![
                                text("Balance:").width(Length::Fixed(120.0)),
                                text(format!("{} sat", addr_info.balance)),
                            ]
                            .spacing(10),
                        ]
                        .spacing(8),
                    )
                    .padding(10)
                    .style(iced::theme::Container::Box),
                );
        }

        content.into()
    }
}
