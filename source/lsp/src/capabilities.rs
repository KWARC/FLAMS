use async_lsp::lsp_types::{self as lsp, CompletionOptions};

pub struct STeXSemanticTokens;
impl STeXSemanticTokens {
    pub const NAME: u32 = 0; // dark blue          light blue  !
    pub const KEYWORD: u32 = 1; // violet             pink        !
    pub const SYMBOL: u32 = 2; // brown              red         !
    pub const DECLARATION: u32 = 3; // Dark Purple        dark brown  !
    pub const REF_MACRO: u32 = 4; // Lime               cyan        !
    pub const VARIABLE: u32 = 5; // ?
    pub const LOCAL: u32 = 6; // light blue         dark blue   !
    // -------------------
    pub const OPERATOR: u32 = 7; // white
    pub const TYPE_PARAMETER: u32 = 8; // math green
    pub const TYPE: u32 = 9; // same
    pub const ENUM: u32 = 10; // same
    pub const MODIFIER: u32 = 11; // yellow / nothing
    pub const COMMENT: u32 = 12; // dark green
    pub const DECORATOR: u32 = 13; // yellow / nothing
}

lazy_static::lazy_static! {
  pub static ref SEMANTIC_TOKENS : lsp::SemanticTokensLegend = lsp::SemanticTokensLegend {
    token_types: vec![
      lsp::SemanticTokenType::ENUM_MEMBER,
      lsp::SemanticTokenType::KEYWORD,
      lsp::SemanticTokenType::STRING,
      lsp::SemanticTokenType::REGEXP,
      lsp::SemanticTokenType::NUMBER,
      lsp::SemanticTokenType::VARIABLE,
      lsp::SemanticTokenType::PROPERTY,
      // ------------------------------
      lsp::SemanticTokenType::OPERATOR,
      lsp::SemanticTokenType::TYPE_PARAMETER,
      lsp::SemanticTokenType::TYPE,
      lsp::SemanticTokenType::ENUM,
      lsp::SemanticTokenType::MODIFIER,
      lsp::SemanticTokenType::COMMENT,
      lsp::SemanticTokenType::DECORATOR,
    ],
    token_modifiers: vec![
      //lsp::SemanticTokenModifier::DEPRECATED,
    ]
  };
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn capabilities() -> lsp::ServerCapabilities {
    lsp::ServerCapabilities {
        position_encoding: Some(lsp::PositionEncodingKind::UTF16),
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Options(
            lsp::TextDocumentSyncOptions {
                open_close: Some(true),
                change: Some(lsp::TextDocumentSyncKind::INCREMENTAL),
                will_save: Some(false),
                will_save_wait_until: Some(false),
                save: Some(lsp::TextDocumentSyncSaveOptions::Supported(true)),
            },
        )),
        semantic_tokens_provider: Some(
            lsp::SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                lsp::SemanticTokensRegistrationOptions {
                    text_document_registration_options: tdro(),
                    semantic_tokens_options: lsp::SemanticTokensOptions {
                        work_done_progress_options: lsp::WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                        range: Some(true),
                        full: Some(lsp::SemanticTokensFullOptions::Delta { delta: Some(true) }),
                        legend: SEMANTIC_TOKENS.clone(),
                    },
                    static_registration_options: lsp::StaticRegistrationOptions {
                        id: Some("stex-sem-tokens".to_string()),
                    },
                },
            ),
        ),
        moniker_provider: Some(lsp::OneOf::Right(
            lsp::MonikerServerCapabilities::RegistrationOptions(lsp::MonikerRegistrationOptions {
                text_document_registration_options: tdro(),
                moniker_options: lsp::MonikerOptions {
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: Some(true),
                    },
                },
            }),
        )),
        document_symbol_provider: Some(lsp::OneOf::Right(lsp::DocumentSymbolOptions {
            label: Some("FLAMS".to_string()),
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        })),
        workspace_symbol_provider: Some(lsp::OneOf::Right(lsp::WorkspaceSymbolOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
            resolve_provider: Some(true),
        })),
        workspace: Some(lsp::WorkspaceServerCapabilities {
            workspace_folders: Some(lsp::WorkspaceFoldersServerCapabilities {
                supported: Some(true),
                change_notifications: Some(lsp::OneOf::Right("flams-change-listener".to_string())),
            }),
            file_operations: Some(lsp::WorkspaceFileOperationsServerCapabilities {
                did_create: Some(lsp::FileOperationRegistrationOptions {
                    filters: fo_filter(),
                }),
                did_rename: Some(lsp::FileOperationRegistrationOptions {
                    filters: fo_filter(),
                }),
                did_delete: Some(lsp::FileOperationRegistrationOptions {
                    filters: fo_filter(),
                }),
                will_create: None,
                will_delete: None,
                will_rename: None,
            }),
        }),
        references_provider: Some(lsp::OneOf::Right(lsp::ReferencesOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        })),
        selection_range_provider: Some(lsp::SelectionRangeProviderCapability::RegistrationOptions(
            lsp::SelectionRangeRegistrationOptions {
                selection_range_options: lsp::SelectionRangeOptions {
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: Some(true),
                    },
                },
                registration_options: lsp::StaticTextDocumentRegistrationOptions {
                    id: Some("stex-selec-range".to_string()),
                    document_selector: Some(doc_filter()),
                },
            },
        )),
        hover_provider: Some(lsp::HoverProviderCapability::Options(lsp::HoverOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        })),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(true),
            trigger_characters: None,
            all_commit_characters: None,
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
            completion_item: Some(lsp::CompletionOptionsCompletionItem {
                label_details_support: Some(true),
            }),
        }),
        signature_help_provider: Some(lsp::SignatureHelpOptions {
            trigger_characters: None,
            retrigger_characters: None,
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        }),
        definition_provider: Some(lsp::OneOf::Right(lsp::DefinitionOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        })),
        type_definition_provider: Some(lsp::TypeDefinitionProviderCapability::Options(
            lsp::StaticTextDocumentRegistrationOptions {
                document_selector: Some(doc_filter()),
                id: Some("stex-type-def".to_string()),
            },
        )),
        implementation_provider: Some(lsp::ImplementationProviderCapability::Options(
            lsp::StaticTextDocumentRegistrationOptions {
                document_selector: Some(doc_filter()),
                id: Some("stex-impl".to_string()),
            },
        )),
        declaration_provider: Some(lsp::DeclarationCapability::RegistrationOptions(
            lsp::DeclarationRegistrationOptions {
                declaration_options: lsp::DeclarationOptions {
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: Some(true),
                    },
                },
                text_document_registration_options: tdro(),
                static_registration_options: lsp::StaticRegistrationOptions {
                    id: Some("stex-decl".to_string()),
                },
            },
        )),
        document_highlight_provider: Some(lsp::OneOf::Right(lsp::DocumentHighlightOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        })),
        code_action_provider: Some(lsp::CodeActionProviderCapability::Options(
            lsp::CodeActionOptions {
                code_action_kinds: Some(vec![
                    lsp::CodeActionKind::QUICKFIX,
                    lsp::CodeActionKind::new("snify/annotate"),
                    lsp::CodeActionKind::REFACTOR,
                    lsp::CodeActionKind::SOURCE,
                    lsp::CodeActionKind::SOURCE_ORGANIZE_IMPORTS,
                    lsp::CodeActionKind::SOURCE_FIX_ALL,
                ]),
                work_done_progress_options: lsp::WorkDoneProgressOptions {
                    work_done_progress: Some(true),
                },
                resolve_provider: Some(true),
            },
        )),
        code_lens_provider: Some(lsp::CodeLensOptions {
            resolve_provider: Some(true),
        }),
        folding_range_provider: Some(lsp::FoldingRangeProviderCapability::Options(
            lsp::StaticTextDocumentColorProviderOptions {
                document_selector: Some(doc_filter()),
                id: Some("stex-folding-range".to_string()),
            },
        )),
        document_link_provider: Some(lsp::DocumentLinkOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
            resolve_provider: Some(true),
        }),
        execute_command_provider: Some(lsp::ExecuteCommandOptions {
            commands: vec![],
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
        }),
        inline_value_provider: Some(lsp::OneOf::Right(
            lsp::InlineValueServerCapabilities::RegistrationOptions(
                lsp::InlineValueRegistrationOptions {
                    text_document_registration_options: tdro(),
                    static_registration_options: lsp::StaticRegistrationOptions {
                        id: Some("stex-inline-value".to_string()),
                    },
                    inline_value_options: lsp::InlineValueOptions {
                        work_done_progress_options: lsp::WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                    },
                },
            ),
        )),
        inlay_hint_provider: Some(lsp::OneOf::Right(
            lsp::InlayHintServerCapabilities::RegistrationOptions(
                lsp::InlayHintRegistrationOptions {
                    text_document_registration_options: tdro(),
                    static_registration_options: lsp::StaticRegistrationOptions {
                        id: Some("stex-inlay-hint".to_string()),
                    },
                    inlay_hint_options: lsp::InlayHintOptions {
                        resolve_provider: Some(false),
                        work_done_progress_options: lsp::WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                    },
                },
            ),
        )),
        diagnostic_provider: Some(lsp::DiagnosticServerCapabilities::RegistrationOptions(
            lsp::DiagnosticRegistrationOptions {
                text_document_registration_options: tdro(),
                static_registration_options: lsp::StaticRegistrationOptions {
                    id: Some("stex-diagnostic".to_string()),
                },
                diagnostic_options: lsp::DiagnosticOptions {
                    work_done_progress_options: lsp::WorkDoneProgressOptions {
                        work_done_progress: Some(true),
                    },
                    identifier: Some("stex-diagnostic".to_string()),
                    inter_file_dependencies: true,
                    workspace_diagnostics: false, // <- this does not seem to work
                },
            },
        )),
        linked_editing_range_provider: Some(
            lsp::LinkedEditingRangeServerCapabilities::RegistrationOptions(
                lsp::LinkedEditingRangeRegistrationOptions {
                    text_document_registration_options: tdro(),
                    static_registration_options: lsp::StaticRegistrationOptions {
                        id: Some("stex-linked-editing-range".to_string()),
                    },
                    linked_editing_range_options: lsp::LinkedEditingRangeOptions {
                        work_done_progress_options: lsp::WorkDoneProgressOptions {
                            work_done_progress: Some(true),
                        },
                    },
                },
            ),
        ),
        rename_provider: Some(lsp::OneOf::Right(lsp::RenameOptions {
            work_done_progress_options: lsp::WorkDoneProgressOptions {
                work_done_progress: Some(true),
            },
            prepare_provider: Some(true),
        })),
        document_formatting_provider: None,
        document_range_formatting_provider: None,
        document_on_type_formatting_provider: None,
        color_provider: None,
        call_hierarchy_provider: Some(lsp::CallHierarchyServerCapability::Simple(true)),
        //inline_completion_provider:None,
        experimental: None,
    }
}

fn tdro() -> lsp::TextDocumentRegistrationOptions {
    lsp::TextDocumentRegistrationOptions {
        document_selector: Some(doc_filter()),
    }
}

fn fo_filter() -> Vec<lsp::FileOperationFilter> {
    vec![lsp::FileOperationFilter {
        scheme: Some("file".to_string()),
        pattern: lsp::FileOperationPattern {
            glob: "**/*.tex".to_string(),
            matches: Some(lsp::FileOperationPatternKind::File),
            options: None,
        },
    }]
}

fn doc_filter() -> Vec<lsp::DocumentFilter> {
    vec![
        lsp::DocumentFilter {
            language: Some("tex".to_string()),
            scheme: Some("file".to_string()),
            pattern: Some("**/*.tex".to_string()),
        },
        lsp::DocumentFilter {
            language: Some("latex".to_string()),
            scheme: Some("file".to_string()),
            pattern: Some("**/*.tex".to_string()),
        },
    ]
}
