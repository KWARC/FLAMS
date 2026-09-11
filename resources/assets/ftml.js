let ftmlViewer = (function(exports) {
    let script_src;
    if (typeof document !== 'undefined' && document.currentScript !== null) {
        script_src = new URL(document.currentScript.src, location.href).toString();
    }

    /**
     * @enum {0 | 1 | 2 | 3}
     */
    const HighlightStyle = Object.freeze({
        Colored: 0, "0": "Colored",
        Subtle: 1, "1": "Subtle",
        Off: 2, "2": "Off",
        None: 3, "3": "None",
    });
    exports.HighlightStyle = HighlightStyle;

    class IntoUnderlyingByteSource {
        __destroy_into_raw() {
            const ptr = this.__wbg_ptr;
            this.__wbg_ptr = 0;
            IntoUnderlyingByteSourceFinalization.unregister(this);
            return ptr;
        }
        free() {
            const ptr = this.__destroy_into_raw();
            wasm.__wbg_intounderlyingbytesource_free(ptr, 0);
        }
        /**
         * @returns {number}
         */
        get autoAllocateChunkSize() {
            const ret = wasm.intounderlyingbytesource_autoAllocateChunkSize(this.__wbg_ptr);
            return ret >>> 0;
        }
        cancel() {
            const ptr = this.__destroy_into_raw();
            wasm.intounderlyingbytesource_cancel(ptr);
        }
        /**
         * @param {ReadableByteStreamController} controller
         * @returns {Promise<any>}
         */
        pull(controller) {
            const ret = wasm.intounderlyingbytesource_pull(this.__wbg_ptr, addHeapObject(controller));
            return takeObject(ret);
        }
        /**
         * @param {ReadableByteStreamController} controller
         */
        start(controller) {
            wasm.intounderlyingbytesource_start(this.__wbg_ptr, addHeapObject(controller));
        }
        /**
         * @returns {ReadableStreamType}
         */
        get type() {
            const ret = wasm.intounderlyingbytesource_type(this.__wbg_ptr);
            return __wbindgen_enum_ReadableStreamType[ret];
        }
    }
    if (Symbol.dispose) IntoUnderlyingByteSource.prototype[Symbol.dispose] = IntoUnderlyingByteSource.prototype.free;
    exports.IntoUnderlyingByteSource = IntoUnderlyingByteSource;

    class IntoUnderlyingSink {
        __destroy_into_raw() {
            const ptr = this.__wbg_ptr;
            this.__wbg_ptr = 0;
            IntoUnderlyingSinkFinalization.unregister(this);
            return ptr;
        }
        free() {
            const ptr = this.__destroy_into_raw();
            wasm.__wbg_intounderlyingsink_free(ptr, 0);
        }
        /**
         * @param {any} reason
         * @returns {Promise<any>}
         */
        abort(reason) {
            const ptr = this.__destroy_into_raw();
            const ret = wasm.intounderlyingsink_abort(ptr, addHeapObject(reason));
            return takeObject(ret);
        }
        /**
         * @returns {Promise<any>}
         */
        close() {
            const ptr = this.__destroy_into_raw();
            const ret = wasm.intounderlyingsink_close(ptr);
            return takeObject(ret);
        }
        /**
         * @param {any} chunk
         * @returns {Promise<any>}
         */
        write(chunk) {
            const ret = wasm.intounderlyingsink_write(this.__wbg_ptr, addHeapObject(chunk));
            return takeObject(ret);
        }
    }
    if (Symbol.dispose) IntoUnderlyingSink.prototype[Symbol.dispose] = IntoUnderlyingSink.prototype.free;
    exports.IntoUnderlyingSink = IntoUnderlyingSink;

    class IntoUnderlyingSource {
        __destroy_into_raw() {
            const ptr = this.__wbg_ptr;
            this.__wbg_ptr = 0;
            IntoUnderlyingSourceFinalization.unregister(this);
            return ptr;
        }
        free() {
            const ptr = this.__destroy_into_raw();
            wasm.__wbg_intounderlyingsource_free(ptr, 0);
        }
        cancel() {
            const ptr = this.__destroy_into_raw();
            wasm.intounderlyingsource_cancel(ptr);
        }
        /**
         * @param {ReadableStreamDefaultController} controller
         * @returns {Promise<any>}
         */
        pull(controller) {
            const ret = wasm.intounderlyingsource_pull(this.__wbg_ptr, addHeapObject(controller));
            return takeObject(ret);
        }
    }
    if (Symbol.dispose) IntoUnderlyingSource.prototype[Symbol.dispose] = IntoUnderlyingSource.prototype.free;
    exports.IntoUnderlyingSource = IntoUnderlyingSource;

    function clear_cache() {
        wasm.clear_cache();
    }
    exports.clear_cache = clear_cache;

    function print_cache() {
        wasm.print_cache();
    }
    exports.print_cache = print_cache;

    function run() {
        wasm.run();
    }
    exports.run = run;
    function __wbg_get_imports() {
        const import0 = {
            __proto__: null,
            __wbg_Error_408e67f47ca7b58b: function(arg0, arg1) {
                const ret = Error(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            },
            __wbg_Number_3890faa6d3ff057d: function(arg0) {
                const ret = Number(getObject(arg0));
                return ret;
            },
            __wbg___wbindgen_bigint_get_as_i64_c4ecf48528083721: function(arg0, arg1) {
                const v = getObject(arg1);
                const ret = typeof(v) === 'bigint' ? v : undefined;
                getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
            },
            __wbg___wbindgen_boolean_get_c9c83ebd41b34df3: function(arg0) {
                const v = getObject(arg0);
                const ret = typeof(v) === 'boolean' ? v : undefined;
                return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
            },
            __wbg___wbindgen_debug_string_a57024b9c6e4a48b: function(arg0, arg1) {
                const ret = debugString(getObject(arg1));
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg___wbindgen_in_ac983077f137f2e6: function(arg0, arg1) {
                const ret = getObject(arg0) in getObject(arg1);
                return ret;
            },
            __wbg___wbindgen_is_bigint_8ffbbef442139384: function(arg0) {
                const ret = typeof(getObject(arg0)) === 'bigint';
                return ret;
            },
            __wbg___wbindgen_is_falsy_b7464e97ddc1b7a4: function(arg0) {
                const ret = !getObject(arg0);
                return ret;
            },
            __wbg___wbindgen_is_function_5e4570eb24ffa122: function(arg0) {
                const ret = typeof(getObject(arg0)) === 'function';
                return ret;
            },
            __wbg___wbindgen_is_null_7d13f41e1a2d5140: function(arg0) {
                const ret = getObject(arg0) === null;
                return ret;
            },
            __wbg___wbindgen_is_null_or_undefined_d3f0c1e48e6f1b85: function(arg0) {
                const ret = getObject(arg0) == null;
                return ret;
            },
            __wbg___wbindgen_is_object_a2790eb24c211ea0: function(arg0) {
                const val = getObject(arg0);
                const ret = typeof(val) === 'object' && val !== null;
                return ret;
            },
            __wbg___wbindgen_is_string_e6f02f0ea5f20a32: function(arg0) {
                const ret = typeof(getObject(arg0)) === 'string';
                return ret;
            },
            __wbg___wbindgen_is_undefined_6cff064c44e0d823: function(arg0) {
                const ret = getObject(arg0) === undefined;
                return ret;
            },
            __wbg___wbindgen_jsval_eq_0a18949a61670320: function(arg0, arg1) {
                const ret = getObject(arg0) === getObject(arg1);
                return ret;
            },
            __wbg___wbindgen_jsval_loose_eq_acf2776254a8d832: function(arg0, arg1) {
                const ret = getObject(arg0) == getObject(arg1);
                return ret;
            },
            __wbg___wbindgen_number_get_136b9679cab35cfb: function(arg0, arg1) {
                const obj = getObject(arg1);
                const ret = typeof(obj) === 'number' ? obj : undefined;
                getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
            },
            __wbg___wbindgen_string_get_d154f1e671052120: function(arg0, arg1) {
                const obj = getObject(arg1);
                const ret = typeof(obj) === 'string' ? obj : undefined;
                var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                var len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg___wbindgen_throw_bb96b2010945f0bc: function(arg0, arg1) {
                throw new Error(getStringFromWasm0(arg0, arg1));
            },
            __wbg__wbg_cb_unref_be22cc64ae6946a0: function(arg0) {
                getObject(arg0)._wbg_cb_unref();
            },
            __wbg_addEventListener_3b8edc02c33d9f77: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3));
            }, arguments); },
            __wbg_addEventListener_795d16a88040eb7f: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).addEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), arg4 !== 0);
            }, arguments); },
            __wbg_add_43c69c0af85151c4: function() { return handleError(function (arg0, arg1, arg2) {
                getObject(arg0).add(getStringFromWasm0(arg1, arg2));
            }, arguments); },
            __wbg_add_519afe2beebf8d55: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).add(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            }, arguments); },
            __wbg_altKey_753fdbc251308d47: function(arg0) {
                const ret = getObject(arg0).altKey;
                return ret;
            },
            __wbg_appendChild_d5cbce3d5fa81471: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).appendChild(getObject(arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_append_96d1baf3bf7864f9: function() { return handleError(function (arg0, arg1) {
                getObject(arg0).append(getObject(arg1));
            }, arguments); },
            __wbg_body_d6eca0586d628e3c: function(arg0) {
                const ret = getObject(arg0).body;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_bottom_364a8da495eed98e: function(arg0) {
                const ret = getObject(arg0).bottom;
                return ret;
            },
            __wbg_buffer_78291c0e094ccf99: function(arg0) {
                const ret = getObject(arg0).buffer;
                return addHeapObject(ret);
            },
            __wbg_byobRequest_f8b1c89429b77545: function(arg0) {
                const ret = getObject(arg0).byobRequest;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_byteLength_336bc7d303511ba0: function(arg0) {
                const ret = getObject(arg0).byteLength;
                return ret;
            },
            __wbg_byteOffset_2b1d5b10453ce198: function(arg0) {
                const ret = getObject(arg0).byteOffset;
                return ret;
            },
            __wbg_call_1c5886ab9c57d1c7: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).call(getObject(arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_call_35dba3c747ad7521: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).call(getObject(arg1), getObject(arg2));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_cancelAnimationFrame_58acec8573d45a99: function() { return handleError(function (arg0, arg1) {
                getObject(arg0).cancelAnimationFrame(arg1);
            }, arguments); },
            __wbg_cancelBubble_3c22a4a11bfa15f9: function(arg0) {
                const ret = getObject(arg0).cancelBubble;
                return ret;
            },
            __wbg_charCodeAt_ee49a2dd698e4f66: function(arg0, arg1) {
                const ret = getObject(arg0).charCodeAt(arg1 >>> 0);
                return ret;
            },
            __wbg_checked_e5cdc0e72e42fdfd: function(arg0) {
                const ret = getObject(arg0).checked;
                return ret;
            },
            __wbg_childNodes_7410f7798ab1eb2b: function(arg0) {
                const ret = getObject(arg0).childNodes;
                return addHeapObject(ret);
            },
            __wbg_classList_23983f0a4979ea93: function(arg0) {
                const ret = getObject(arg0).classList;
                return addHeapObject(ret);
            },
            __wbg_clearTimeout_4e61cface6c91ad9: function(arg0, arg1) {
                getObject(arg0).clearTimeout(arg1);
            },
            __wbg_clientWidth_ad03e8eb6c2b0c56: function(arg0) {
                const ret = getObject(arg0).clientWidth;
                return ret;
            },
            __wbg_clientX_bf415eaddddbd22c: function(arg0) {
                const ret = getObject(arg0).clientX;
                return ret;
            },
            __wbg_clientY_406b7a2827c06e05: function(arg0) {
                const ret = getObject(arg0).clientY;
                return ret;
            },
            __wbg_cloneNode_1c667adc0c119cfa: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).cloneNode();
                return addHeapObject(ret);
            }, arguments); },
            __wbg_cloneNode_dac33f79a25a9355: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).cloneNode(arg1 !== 0);
                return addHeapObject(ret);
            }, arguments); },
            __wbg_close_72f69f5f2de2bc73: function() { return handleError(function (arg0) {
                getObject(arg0).close();
            }, arguments); },
            __wbg_close_97cdb44c3a7878f6: function() { return handleError(function (arg0) {
                getObject(arg0).close();
            }, arguments); },
            __wbg_code_1bac1fd03147d97e: function(arg0, arg1) {
                const ret = getObject(arg1).code;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_commonAncestorContainer_bf669a004fe1f9e7: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).commonAncestorContainer;
                return addHeapObject(ret);
            }, arguments); },
            __wbg_composedPath_fc8c6c0a810bd2dc: function(arg0) {
                const ret = getObject(arg0).composedPath();
                return addHeapObject(ret);
            },
            __wbg_construct_a5c4a12c650f2c30: function() { return handleError(function (arg0, arg1) {
                const ret = Reflect.construct(getObject(arg0), getObject(arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_contains_db96d32718835087: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).contains(getStringFromWasm0(arg1, arg2));
                return ret;
            },
            __wbg_content_78a37721789ada5f: function(arg0) {
                const ret = getObject(arg0).content;
                return addHeapObject(ret);
            },
            __wbg_createComment_8bb21f73bb03ef86: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).createComment(getStringFromWasm0(arg1, arg2));
                return addHeapObject(ret);
            },
            __wbg_createElementNS_f18ede2d74f15ea1: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                const ret = getObject(arg0).createElementNS(arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_createElement_7f42344eee7bb810: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).createElement(getStringFromWasm0(arg1, arg2));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_createTextNode_f5ee2b1cd3e249bb: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).createTextNode(getStringFromWasm0(arg1, arg2));
                return addHeapObject(ret);
            },
            __wbg_createTreeWalker_75e543638fab345f: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                const ret = getObject(arg0).createTreeWalker(getObject(arg1), arg2 >>> 0, getObject(arg3));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_ctrlKey_8f6cb44d63052c81: function(arg0) {
                const ret = getObject(arg0).ctrlKey;
                return ret;
            },
            __wbg_deleteProperty_83dd9487ca70fb9c: function() { return handleError(function (arg0, arg1) {
                const ret = Reflect.deleteProperty(getObject(arg0), getObject(arg1));
                return ret;
            }, arguments); },
            __wbg_documentElement_5db4560019067e32: function(arg0) {
                const ret = getObject(arg0).documentElement;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_document_ac38448dbfd31a57: function(arg0) {
                const ret = getObject(arg0).document;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_done_669171204c3dcae2: function(arg0) {
                const ret = getObject(arg0).done;
                return ret;
            },
            __wbg_enqueue_7d68a21eda78e72f: function() { return handleError(function (arg0, arg1) {
                getObject(arg0).enqueue(getObject(arg1));
            }, arguments); },
            __wbg_entries_7774d489e1da5f4f: function(arg0) {
                const ret = Object.entries(getObject(arg0));
                return addHeapObject(ret);
            },
            __wbg_error_757e9472f8410341: function(arg0, arg1) {
                let deferred0_0;
                let deferred0_1;
                try {
                    deferred0_0 = arg0;
                    deferred0_1 = arg1;
                    console.error(getStringFromWasm0(arg0, arg1));
                } finally {
                    wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
                }
            },
            __wbg_error_dd408a7b3cb542dd: function(arg0) {
                console.error(getObject(arg0));
            },
            __wbg_exec_38ad8544f3498edd: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).exec(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_fetch_9b478faef8cda538: function(arg0) {
                const ret = fetch(getObject(arg0));
                return addHeapObject(ret);
            },
            __wbg_firstChild_078b3f58c509c470: function(arg0) {
                const ret = getObject(arg0).firstChild;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_firstChild_f76da00a56ec41d4: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).firstChild();
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_firstElementChild_c133913f3927264c: function(arg0) {
                const ret = getObject(arg0).firstElementChild;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_focus_77d7483c7b2b9f30: function() { return handleError(function (arg0) {
                getObject(arg0).focus();
            }, arguments); },
            __wbg_fromEntries_464704b0ede47aaf: function() { return handleError(function (arg0) {
                const ret = Object.fromEntries(getObject(arg0));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_fullscreenElement_f225b88d410fadc1: function(arg0) {
                const ret = getObject(arg0).fullscreenElement;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_getAttributeNames_e6c893fc08db79c0: function(arg0) {
                const ret = getObject(arg0).getAttributeNames();
                return addHeapObject(ret);
            },
            __wbg_getAttribute_4c6e1df05f9ee034: function(arg0, arg1, arg2, arg3) {
                const ret = getObject(arg1).getAttribute(getStringFromWasm0(arg2, arg3));
                var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                var len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_getBoundingClientRect_8165472d0fc91753: function(arg0) {
                const ret = getObject(arg0).getBoundingClientRect();
                return addHeapObject(ret);
            },
            __wbg_getComputedStyle_d0f299f3151ac25e: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).getComputedStyle(getObject(arg1));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_getElementById_1637d6969b003cda: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).getElementById(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_getItem_eb388fb8c39edb35: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                const ret = getObject(arg1).getItem(getStringFromWasm0(arg2, arg3));
                var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                var len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            }, arguments); },
            __wbg_getPropertyValue_50144438fb4fc8f4: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                const ret = getObject(arg1).getPropertyValue(getStringFromWasm0(arg2, arg3));
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            }, arguments); },
            __wbg_getRandomValues_2b907d0af5db96ee: function() { return handleError(function (arg0, arg1) {
                globalThis.crypto.getRandomValues(getArrayU8FromWasm0(arg0, arg1));
            }, arguments); },
            __wbg_getRangeAt_b340f9f994bce286: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).getRangeAt(arg1 >>> 0);
                return addHeapObject(ret);
            }, arguments); },
            __wbg_getSelection_214893a2faa774e3: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).getSelection();
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_getTime_63fb0332e6c4ec17: function(arg0) {
                const ret = getObject(arg0).getTime();
                return ret;
            },
            __wbg_getTimezoneOffset_4baa793e0d3962a8: function(arg0) {
                const ret = getObject(arg0).getTimezoneOffset();
                return ret;
            },
            __wbg_get_01667a0edd483167: function() { return handleError(function (arg0, arg1) {
                const ret = Reflect.get(getObject(arg0), arg1 >>> 0);
                return addHeapObject(ret);
            }, arguments); },
            __wbg_get_36debceb6d43d7a1: function(arg0, arg1) {
                const ret = getObject(arg0)[arg1 >>> 0];
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_get_971a0c45d172643f: function() { return handleError(function (arg0, arg1) {
                const ret = Reflect.get(getObject(arg0), getObject(arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_get_c0c8f8d7da0c03dd: function(arg0, arg1) {
                const ret = getObject(arg0)[arg1 >>> 0];
                return addHeapObject(ret);
            },
            __wbg_get_d173c0308df22d37: function() { return handleError(function (arg0, arg1) {
                const ret = Reflect.get(getObject(arg0), getObject(arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_get_unchecked_e20b893aeafc3fca: function(arg0, arg1) {
                const ret = getObject(arg0)[arg1 >>> 0];
                return addHeapObject(ret);
            },
            __wbg_get_with_ref_key_6412cf3094599694: function(arg0, arg1) {
                const ret = getObject(arg0)[getObject(arg1)];
                return addHeapObject(ret);
            },
            __wbg_hasAttribute_bd49862fb0eb1701: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).hasAttribute(getStringFromWasm0(arg1, arg2));
                return ret;
            },
            __wbg_hash_597d991faa794205: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg1).hash;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            }, arguments); },
            __wbg_head_6c0efd90f4024307: function(arg0) {
                const ret = getObject(arg0).head;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_height_949ea86de6d5d939: function(arg0) {
                const ret = getObject(arg0).height;
                return ret;
            },
            __wbg_host_f512e97ce1222138: function(arg0) {
                const ret = getObject(arg0).host;
                return addHeapObject(ret);
            },
            __wbg_id_10497a73745167b0: function(arg0, arg1) {
                const ret = getObject(arg1).id;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_includes_a4b83ade703cb80b: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).includes(getObject(arg1), arg2);
                return ret;
            },
            __wbg_innerHeight_047a04fc237d09df: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).innerHeight;
                return addHeapObject(ret);
            }, arguments); },
            __wbg_innerWidth_4a973110eeb5f463: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).innerWidth;
                return addHeapObject(ret);
            }, arguments); },
            __wbg_insertBefore_f3733e91a030079e: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).insertBefore(getObject(arg1), getObject(arg2));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_instanceof_ArrayBuffer_993d02d2d254cad1: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof ArrayBuffer;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Element_a3960bb00f4964bc: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Element;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Error_61d8a02a0f3383a1: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Error;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_HtmlElement_6b02a3740edba922: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof HTMLElement;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_HtmlInputElement_6077656bcaf1eb33: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof HTMLInputElement;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_KeyboardEvent_ab6e9b5a4d8bf1db: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof KeyboardEvent;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Map_9a4d6ead180ae3a9: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Map;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Node_ad9597995317f467: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Node;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_RegExp_c7ed8b5072ff70f1: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof RegExp;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Response_8f49efbd4bfd76d6: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Response;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_ShadowRoot_55844b1b54688323: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof ShadowRoot;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Uint8Array_f935dbb0aa7cdeed: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Uint8Array;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_instanceof_Window_5625ff9937037a38: function(arg0) {
                let result;
                try {
                    result = getObject(arg0) instanceof Window;
                } catch (_) {
                    result = false;
                }
                const ret = result;
                return ret;
            },
            __wbg_isArray_291e8fbbc73f8b2e: function(arg0) {
                const ret = Array.isArray(getObject(arg0));
                return ret;
            },
            __wbg_isArray_6339f732981044bf: function(arg0) {
                const ret = Array.isArray(getObject(arg0));
                return ret;
            },
            __wbg_isSafeInteger_f3d6cd19ccfe4512: function(arg0) {
                const ret = Number.isSafeInteger(getObject(arg0));
                return ret;
            },
            __wbg_is_86be747e88e872fb: function(arg0, arg1) {
                const ret = Object.is(getObject(arg0), getObject(arg1));
                return ret;
            },
            __wbg_iterator_5cebbb86e33c6dd6: function() {
                const ret = Symbol.iterator;
                return addHeapObject(ret);
            },
            __wbg_keyCode_9e78ef01aed14df6: function(arg0) {
                const ret = getObject(arg0).keyCode;
                return ret;
            },
            __wbg_key_d1b2fd5ee42567c0: function(arg0, arg1) {
                const ret = getObject(arg1).key;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_lastChild_0a7d8f8eae311945: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).lastChild();
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_left_5acbc01043b87b48: function(arg0) {
                const ret = getObject(arg0).left;
                return ret;
            },
            __wbg_length_36bd29c6848c2144: function(arg0) {
                const ret = getObject(arg0).length;
                return ret;
            },
            __wbg_length_7afd83ae4ddf324f: function(arg0) {
                const ret = getObject(arg0).length;
                return ret;
            },
            __wbg_length_ecfa2c63d3d0d82c: function(arg0) {
                const ret = getObject(arg0).length;
                return ret;
            },
            __wbg_localStorage_19bddab1e4cb2413: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).localStorage;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_location_5d269cf0aa99107a: function(arg0) {
                const ret = getObject(arg0).location;
                return addHeapObject(ret);
            },
            __wbg_log_1f8cbb01c83d06c2: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
                let deferred0_0;
                let deferred0_1;
                try {
                    deferred0_0 = arg0;
                    deferred0_1 = arg1;
                    console.log(getStringFromWasm0(arg0, arg1), getStringFromWasm0(arg2, arg3), getStringFromWasm0(arg4, arg5), getStringFromWasm0(arg6, arg7));
                } finally {
                    wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
                }
            },
            __wbg_log_a54ca6b45e09078a: function(arg0, arg1) {
                let deferred0_0;
                let deferred0_1;
                try {
                    deferred0_0 = arg0;
                    deferred0_1 = arg1;
                    console.log(getStringFromWasm0(arg0, arg1));
                } finally {
                    wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
                }
            },
            __wbg_log_e6372b4fbfc9f81e: function(arg0) {
                console.log(getObject(arg0));
            },
            __wbg_mark_6b7f03786f5e4d61: function(arg0, arg1) {
                performance.mark(getStringFromWasm0(arg0, arg1));
            },
            __wbg_measure_0e21b33a1c6e3a29: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                let deferred0_0;
                let deferred0_1;
                let deferred1_0;
                let deferred1_1;
                try {
                    deferred0_0 = arg0;
                    deferred0_1 = arg1;
                    deferred1_0 = arg2;
                    deferred1_1 = arg3;
                    performance.measure(getStringFromWasm0(arg0, arg1), getStringFromWasm0(arg2, arg3));
                } finally {
                    wasm.__wbindgen_export4(deferred0_0, deferred0_1, 1);
                    wasm.__wbindgen_export4(deferred1_0, deferred1_1, 1);
                }
            }, arguments); },
            __wbg_message_c141d5e68716b595: function(arg0) {
                const ret = getObject(arg0).message;
                return addHeapObject(ret);
            },
            __wbg_metaKey_917f037461143e51: function(arg0) {
                const ret = getObject(arg0).metaKey;
                return ret;
            },
            __wbg_name_7adfb7f7f1539878: function(arg0) {
                const ret = getObject(arg0).name;
                return addHeapObject(ret);
            },
            __wbg_new_0_f117d868b403dc07: function() {
                const ret = new Date();
                return addHeapObject(ret);
            },
            __wbg_new_227d7c05414eb861: function() {
                const ret = new Error();
                return addHeapObject(ret);
            },
            __wbg_new_358857d90afd5a2d: function(arg0, arg1) {
                const ret = new Error(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            },
            __wbg_new_77cc4f4f472aeb81: function(arg0) {
                const ret = new Uint8Array(getObject(arg0));
                return addHeapObject(ret);
            },
            __wbg_new_95039e162b0c4466: function() { return handleError(function () {
                const ret = new Headers();
                return addHeapObject(ret);
            }, arguments); },
            __wbg_new_b4f72b5144efe03b: function() { return handleError(function () {
                const ret = new URLSearchParams();
                return addHeapObject(ret);
            }, arguments); },
            __wbg_new_c4a5c3368506feb1: function() { return handleError(function (arg0, arg1) {
                const ret = new URL(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_new_ebe3e0f6837f0879: function() {
                const ret = new Object();
                return addHeapObject(ret);
            },
            __wbg_new_f4416560103d8b50: function(arg0, arg1, arg2, arg3) {
                const ret = new RegExp(getStringFromWasm0(arg0, arg1), getStringFromWasm0(arg2, arg3));
                return addHeapObject(ret);
            },
            __wbg_new_f9d6489212f3b2b3: function(arg0) {
                const ret = new Date(getObject(arg0));
                return addHeapObject(ret);
            },
            __wbg_new_typed_cceaf62d8d95e9f2: function(arg0, arg1) {
                try {
                    var state0 = {a: arg0, b: arg1};
                    var cb0 = (arg0, arg1) => {
                        const a = state0.a;
                        state0.a = 0;
                        try {
                            return __wasm_bindgen_func_elem_33561(a, state0.b, arg0, arg1);
                        } finally {
                            state0.a = a;
                        }
                    };
                    const ret = new Promise(cb0);
                    return addHeapObject(ret);
                } finally {
                    state0.a = 0;
                }
            },
            __wbg_new_with_args_75f20e1087b74fa8: function(arg0, arg1, arg2, arg3) {
                const ret = new Function(getStringFromWasm0(arg0, arg1), getStringFromWasm0(arg2, arg3));
                return addHeapObject(ret);
            },
            __wbg_new_with_byte_offset_and_length_ff6e927f8d72f0c3: function(arg0, arg1, arg2) {
                const ret = new Uint8Array(getObject(arg0), arg1 >>> 0, arg2 >>> 0);
                return addHeapObject(ret);
            },
            __wbg_new_with_length_2ccc5dbfb4541247: function(arg0) {
                const ret = new Array(arg0 >>> 0);
                return addHeapObject(ret);
            },
            __wbg_new_with_str_4ebb1146a76a8593: function() { return handleError(function (arg0, arg1) {
                const ret = new Request(getStringFromWasm0(arg0, arg1));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_new_with_str_and_init_5a37d576dec75a86: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = new Request(getStringFromWasm0(arg0, arg1), getObject(arg2));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_new_with_year_month_day_hr_min_sec_9659abbdf307aa7c: function(arg0, arg1, arg2, arg3, arg4, arg5) {
                const ret = new Date(arg0 >>> 0, arg1, arg2, arg3, arg4, arg5);
                return addHeapObject(ret);
            },
            __wbg_nextNode_efb2cf2fff4ea54b: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).nextNode();
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_nextSibling_1270411ea2610f57: function(arg0) {
                const ret = getObject(arg0).nextSibling;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_next_42cf16ee0dafc9e2: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).next();
                return addHeapObject(ret);
            }, arguments); },
            __wbg_next_8f26b64fa5e9f64b: function(arg0) {
                const ret = getObject(arg0).next;
                return addHeapObject(ret);
            },
            __wbg_nodeType_650833cf69444d17: function(arg0) {
                const ret = getObject(arg0).nodeType;
                return ret;
            },
            __wbg_offsetHeight_3ac5973ad1baf528: function(arg0) {
                const ret = getObject(arg0).offsetHeight;
                return ret;
            },
            __wbg_offsetWidth_5d9c7950b81e03f2: function(arg0) {
                const ret = getObject(arg0).offsetWidth;
                return ret;
            },
            __wbg_outerHTML_6b872f67d4531f96: function(arg0, arg1) {
                const ret = getObject(arg1).outerHTML;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_ownKeys_49880e0197268893: function() { return handleError(function (arg0) {
                const ret = Reflect.ownKeys(getObject(arg0));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_parentElement_ef76606593484767: function(arg0) {
                const ret = getObject(arg0).parentElement;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_parentNode_8634e029370ec1bb: function(arg0) {
                const ret = getObject(arg0).parentNode;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_prepend_8876422fb2d19556: function() { return handleError(function (arg0, arg1) {
                getObject(arg0).prepend(getObject(arg1));
            }, arguments); },
            __wbg_preventDefault_19878c58b8010668: function(arg0) {
                getObject(arg0).preventDefault();
            },
            __wbg_previousNode_d3aa4baac299498c: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).previousNode();
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_prototypesetcall_de8e0d9553586985: function(arg0, arg1, arg2) {
                Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), getObject(arg2));
            },
            __wbg_querySelector_2c472eddb417c6b3: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).querySelector(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_querySelector_839d6534e69c0f64: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).querySelector(getStringFromWasm0(arg1, arg2));
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            }, arguments); },
            __wbg_queueMicrotask_ac694eae12e92dfb: function(arg0) {
                queueMicrotask(getObject(arg0));
            },
            __wbg_queueMicrotask_be5fe34a8f4cad4d: function(arg0) {
                const ret = getObject(arg0).queueMicrotask;
                return addHeapObject(ret);
            },
            __wbg_readyState_3abe89507fcd616d: function(arg0, arg1) {
                const ret = getObject(arg1).readyState;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_removeAttribute_bb10532a6f012605: function() { return handleError(function (arg0, arg1, arg2) {
                getObject(arg0).removeAttribute(getStringFromWasm0(arg1, arg2));
            }, arguments); },
            __wbg_removeEventListener_aa653c6b402cc27e: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3));
            }, arguments); },
            __wbg_removeEventListener_f0778286eef3aecc: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).removeEventListener(getStringFromWasm0(arg1, arg2), getObject(arg3), arg4 !== 0);
            }, arguments); },
            __wbg_removeItem_a7bebfec650435c7: function() { return handleError(function (arg0, arg1, arg2) {
                getObject(arg0).removeItem(getStringFromWasm0(arg1, arg2));
            }, arguments); },
            __wbg_removeProperty_cdd2665e76b8f1c6: function() { return handleError(function (arg0, arg1, arg2, arg3) {
                const ret = getObject(arg1).removeProperty(getStringFromWasm0(arg2, arg3));
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            }, arguments); },
            __wbg_remove_07453fe173d20eee: function(arg0) {
                getObject(arg0).remove();
            },
            __wbg_remove_426d5806a1a02ede: function() { return handleError(function (arg0, arg1, arg2) {
                getObject(arg0).remove(getStringFromWasm0(arg1, arg2));
            }, arguments); },
            __wbg_remove_761636ea7d78581e: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).remove(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            }, arguments); },
            __wbg_remove_b09405b87e5f3b07: function(arg0) {
                getObject(arg0).remove();
            },
            __wbg_requestAnimationFrame_bcb3ce6247e27dd4: function() { return handleError(function (arg0, arg1) {
                const ret = getObject(arg0).requestAnimationFrame(getObject(arg1));
                return ret;
            }, arguments); },
            __wbg_requestFullscreen_fde993af7528dc3c: function() { return handleError(function (arg0) {
                getObject(arg0).requestFullscreen();
            }, arguments); },
            __wbg_resolve_020f95d838c6ef25: function(arg0) {
                const ret = Promise.resolve(getObject(arg0));
                return addHeapObject(ret);
            },
            __wbg_respond_f88cbcebace42068: function() { return handleError(function (arg0, arg1) {
                getObject(arg0).respond(arg1 >>> 0);
            }, arguments); },
            __wbg_right_6340535ef1da182e: function(arg0) {
                const ret = getObject(arg0).right;
                return ret;
            },
            __wbg_root_702f73dfb1c0703a: function(arg0) {
                const ret = getObject(arg0).root;
                return addHeapObject(ret);
            },
            __wbg_scrollIntoView_35690d3c440e4767: function(arg0, arg1) {
                getObject(arg0).scrollIntoView(arg1 !== 0);
            },
            __wbg_scrollIntoView_49a46961bbda8765: function(arg0, arg1) {
                getObject(arg0).scrollIntoView(getObject(arg1));
            },
            __wbg_scrollLeft_992b059445f1c670: function(arg0) {
                const ret = getObject(arg0).scrollLeft;
                return ret;
            },
            __wbg_scrollTop_af6534cf7a37fb55: function(arg0) {
                const ret = getObject(arg0).scrollTop;
                return ret;
            },
            __wbg_scrollX_181b5c8bcd3df278: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).scrollX;
                return ret;
            }, arguments); },
            __wbg_scrollY_47193160b143bbe9: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).scrollY;
                return ret;
            }, arguments); },
            __wbg_search_3ab40a92dceeaacb: function(arg0, arg1) {
                const ret = getObject(arg1).search;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_setAttribute_507f8367905a9c03: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            }, arguments); },
            __wbg_setItem_b0bb6a578106db69: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).setItem(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            }, arguments); },
            __wbg_setProperty_684ce273e28a7037: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
                getObject(arg0).setProperty(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            }, arguments); },
            __wbg_setTimeout_8be4960d8ad2bb76: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = getObject(arg0).setTimeout(getObject(arg1), arg2);
                return ret;
            }, arguments); },
            __wbg_set_8155bb79a948541b: function() { return handleError(function (arg0, arg1, arg2) {
                const ret = Reflect.set(getObject(arg0), getObject(arg1), getObject(arg2));
                return ret;
            }, arguments); },
            __wbg_set_a80955eb93b145c6: function(arg0, arg1, arg2) {
                getObject(arg0)[arg1 >>> 0] = takeObject(arg2);
            },
            __wbg_set_accept_node_f7ca597245a1ff7c: function(arg0, arg1) {
                getObject(arg0).acceptNode = getObject(arg1);
            },
            __wbg_set_b9b5b5cb7b495037: function(arg0, arg1, arg2) {
                getObject(arg0).set(getArrayU8FromWasm0(arg1, arg2));
            },
            __wbg_set_behavior_3b9d875369f000f4: function(arg0, arg1) {
                getObject(arg0).behavior = __wbindgen_enum_ScrollBehavior[arg1];
            },
            __wbg_set_block_184aed92bb2b5929: function(arg0, arg1) {
                getObject(arg0).block = __wbindgen_enum_ScrollLogicalPosition[arg1];
            },
            __wbg_set_body_f301b68bff45f419: function(arg0, arg1) {
                getObject(arg0).body = getObject(arg1);
            },
            __wbg_set_currentNode_933519ef3826e78c: function(arg0, arg1) {
                getObject(arg0).currentNode = getObject(arg1);
            },
            __wbg_set_headers_805555608daf7f2a: function(arg0, arg1) {
                getObject(arg0).headers = getObject(arg1);
            },
            __wbg_set_innerHTML_7d84b81d6f2a9fdf: function(arg0, arg1, arg2) {
                getObject(arg0).innerHTML = getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_method_cf2b992b9a610bc3: function(arg0, arg1, arg2) {
                getObject(arg0).method = getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_nodeValue_9b1ff418691c2d97: function(arg0, arg1, arg2) {
                getObject(arg0).nodeValue = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_scrollLeft_24e5760b4445ba27: function(arg0, arg1) {
                getObject(arg0).scrollLeft = arg1;
            },
            __wbg_set_scrollTop_b53eccdf1d8574f5: function(arg0, arg1) {
                getObject(arg0).scrollTop = arg1;
            },
            __wbg_set_search_65c58ba6f17e037f: function(arg0, arg1, arg2) {
                getObject(arg0).search = getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_textContent_e027901c7bc836b5: function(arg0, arg1, arg2) {
                getObject(arg0).textContent = arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_value_a1b6eb7fc1095562: function(arg0, arg1, arg2) {
                getObject(arg0).value = getStringFromWasm0(arg1, arg2);
            },
            __wbg_set_x_8c77978914f5cd0d: function(arg0, arg1) {
                getObject(arg0).x = arg1;
            },
            __wbg_set_y_f3a4fb17581748fd: function(arg0, arg1) {
                getObject(arg0).y = arg1;
            },
            __wbg_slice_cc0f63edb5397a2d: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).slice(arg1 >>> 0, arg2 >>> 0);
                return addHeapObject(ret);
            },
            __wbg_stack_3b0d974bbf31e44f: function(arg0, arg1) {
                const ret = getObject(arg1).stack;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_static_accessor_GLOBAL_THIS_466428f93b4eaa76: function() {
                const ret = typeof globalThis === 'undefined' ? null : globalThis;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_static_accessor_GLOBAL_c7aea38d4de089bc: function() {
                const ret = typeof global === 'undefined' ? null : global;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_static_accessor_SELF_42d4fae05e59267a: function() {
                const ret = typeof self === 'undefined' ? null : self;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_static_accessor_WINDOW_e0db14a0eba6a812: function() {
                const ret = typeof window === 'undefined' ? null : window;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_status_b0de02a07fd7d927: function(arg0) {
                const ret = getObject(arg0).status;
                return ret;
            },
            __wbg_stopPropagation_13b9aec9ab20a836: function(arg0) {
                getObject(arg0).stopPropagation();
            },
            __wbg_stringify_f93a4ebae9231922: function() { return handleError(function (arg0) {
                const ret = JSON.stringify(getObject(arg0));
                return addHeapObject(ret);
            }, arguments); },
            __wbg_style_f09d6445af3dd2c6: function(arg0) {
                const ret = getObject(arg0).style;
                return addHeapObject(ret);
            },
            __wbg_tagName_d9b3dd11c1a47e27: function(arg0, arg1) {
                const ret = getObject(arg1).tagName;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_target_13424fe1cdc436ac: function(arg0) {
                const ret = getObject(arg0).target;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_textContent_a8ab419abd77b63c: function(arg0, arg1) {
                const ret = getObject(arg1).textContent;
                var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                var len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_text_9302f33ea8cfce7b: function() { return handleError(function (arg0) {
                const ret = getObject(arg0).text();
                return addHeapObject(ret);
            }, arguments); },
            __wbg_then_7026b513a94278a8: function(arg0, arg1) {
                const ret = getObject(arg0).then(getObject(arg1));
                return addHeapObject(ret);
            },
            __wbg_then_72819b8d4e081fb5: function(arg0, arg1, arg2) {
                const ret = getObject(arg0).then(getObject(arg1), getObject(arg2));
                return addHeapObject(ret);
            },
            __wbg_toString_033acf19ce89359c: function(arg0) {
                const ret = getObject(arg0).toString();
                return addHeapObject(ret);
            },
            __wbg_toString_2f0b0aec069cb718: function(arg0) {
                const ret = getObject(arg0).toString();
                return addHeapObject(ret);
            },
            __wbg_top_66d56bb4eca7d9c5: function(arg0) {
                const ret = getObject(arg0).top;
                return ret;
            },
            __wbg_url_3e90676c7072325d: function(arg0, arg1) {
                const ret = getObject(arg1).url;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_value_1e2369fab29b420e: function(arg0) {
                const ret = getObject(arg0).value;
                return addHeapObject(ret);
            },
            __wbg_value_35f0fb42e7c3d468: function(arg0, arg1) {
                const ret = getObject(arg1).value;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_value_ea3f13bcabcbe7ca: function(arg0, arg1) {
                const ret = getObject(arg1).value;
                const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_export, wasm.__wbindgen_export2);
                const len1 = WASM_VECTOR_LEN;
                getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
                getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
            },
            __wbg_view_7685fe4b2845c5b6: function(arg0) {
                const ret = getObject(arg0).view;
                return isLikeNone(ret) ? 0 : addHeapObject(ret);
            },
            __wbg_width_9eed45149c0366f2: function(arg0) {
                const ret = getObject(arg0).width;
                return ret;
            },
            __wbg_x_e6bb472fdb2dfc97: function(arg0) {
                const ret = getObject(arg0).x;
                return ret;
            },
            __wbg_y_5cf9c01f3aa8e124: function(arg0) {
                const ret = getObject(arg0).y;
                return ret;
            },
            __wbindgen_cast_0000000000000001: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 6782, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
                const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_28472);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000002: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 7060, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
                const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_33559);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000003: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Event")], shim_idx: 1934, ret: Unit, inner_ret: Some(Unit) }, mutable: false }) -> Externref`.
                const ret = makeClosure(arg0, arg1, __wasm_bindgen_func_elem_9958);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000004: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Event")], shim_idx: 6782, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
                const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_28472_3);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000005: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("MouseEvent")], shim_idx: 5221, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
                const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_19450);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000006: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [NamedExternref("Node")], shim_idx: 5965, ret: U32, inner_ret: Some(U32) }, mutable: false }) -> Externref`.
                const ret = makeClosure(arg0, arg1, __wasm_bindgen_func_elem_24768);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000007: function(arg0, arg1) {
                // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [], shim_idx: 6781, ret: Unit, inner_ret: Some(Unit) }, mutable: true }) -> Externref`.
                const ret = makeMutClosure(arg0, arg1, __wasm_bindgen_func_elem_28471);
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000008: function(arg0) {
                // Cast intrinsic for `F64 -> Externref`.
                const ret = arg0;
                return addHeapObject(ret);
            },
            __wbindgen_cast_0000000000000009: function(arg0) {
                // Cast intrinsic for `I64 -> Externref`.
                const ret = arg0;
                return addHeapObject(ret);
            },
            __wbindgen_cast_000000000000000a: function(arg0, arg1) {
                // Cast intrinsic for `Ref(String) -> Externref`.
                const ret = getStringFromWasm0(arg0, arg1);
                return addHeapObject(ret);
            },
            __wbindgen_cast_000000000000000b: function(arg0) {
                // Cast intrinsic for `U64 -> Externref`.
                const ret = BigInt.asUintN(64, arg0);
                return addHeapObject(ret);
            },
            __wbindgen_object_clone_ref: function(arg0) {
                const ret = getObject(arg0);
                return addHeapObject(ret);
            },
            __wbindgen_object_drop_ref: function(arg0) {
                takeObject(arg0);
            },
        };
        return {
            __proto__: null,
            "./ftml_bg.js": import0,
        };
    }

    function __wasm_bindgen_func_elem_28471(arg0, arg1) {
        wasm.__wasm_bindgen_func_elem_28471(arg0, arg1);
    }

    function __wasm_bindgen_func_elem_28472(arg0, arg1, arg2) {
        wasm.__wasm_bindgen_func_elem_28472(arg0, arg1, addHeapObject(arg2));
    }

    function __wasm_bindgen_func_elem_9958(arg0, arg1, arg2) {
        wasm.__wasm_bindgen_func_elem_9958(arg0, arg1, addHeapObject(arg2));
    }

    function __wasm_bindgen_func_elem_28472_3(arg0, arg1, arg2) {
        wasm.__wasm_bindgen_func_elem_28472_3(arg0, arg1, addHeapObject(arg2));
    }

    function __wasm_bindgen_func_elem_19450(arg0, arg1, arg2) {
        wasm.__wasm_bindgen_func_elem_19450(arg0, arg1, addHeapObject(arg2));
    }

    function __wasm_bindgen_func_elem_24768(arg0, arg1, arg2) {
        const ret = wasm.__wasm_bindgen_func_elem_24768(arg0, arg1, addHeapObject(arg2));
        return ret >>> 0;
    }

    function __wasm_bindgen_func_elem_33559(arg0, arg1, arg2) {
        try {
            const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
            wasm.__wasm_bindgen_func_elem_33559(retptr, arg0, arg1, addHeapObject(arg2));
            var r0 = getDataViewMemory0().getInt32(retptr + 4 * 0, true);
            var r1 = getDataViewMemory0().getInt32(retptr + 4 * 1, true);
            if (r1) {
                throw takeObject(r0);
            }
        } finally {
            wasm.__wbindgen_add_to_stack_pointer(16);
        }
    }

    function __wasm_bindgen_func_elem_33561(arg0, arg1, arg2, arg3) {
        wasm.__wasm_bindgen_func_elem_33561(arg0, arg1, addHeapObject(arg2), addHeapObject(arg3));
    }


    const __wbindgen_enum_ReadableStreamType = ["bytes"];


    const __wbindgen_enum_ScrollBehavior = ["auto", "instant", "smooth"];


    const __wbindgen_enum_ScrollLogicalPosition = ["start", "center", "end", "nearest"];
    const IntoUnderlyingByteSourceFinalization = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(ptr => wasm.__wbg_intounderlyingbytesource_free(ptr, 1));
    const IntoUnderlyingSinkFinalization = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(ptr => wasm.__wbg_intounderlyingsink_free(ptr, 1));
    const IntoUnderlyingSourceFinalization = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(ptr => wasm.__wbg_intounderlyingsource_free(ptr, 1));

    function addHeapObject(obj) {
        if (heap_next === heap.length) heap.push(heap.length + 1);
        const idx = heap_next;
        heap_next = heap[idx];

        heap[idx] = obj;
        return idx;
    }

    const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
        ? { register: () => {}, unregister: () => {} }
        : new FinalizationRegistry(state => wasm.__wbindgen_export5(state.a, state.b));

    function debugString(val) {
        // primitive types
        const type = typeof val;
        if (type == 'number' || type == 'boolean' || val == null) {
            return  `${val}`;
        }
        if (type == 'string') {
            return `"${val}"`;
        }
        if (type == 'symbol') {
            const description = val.description;
            if (description == null) {
                return 'Symbol';
            } else {
                return `Symbol(${description})`;
            }
        }
        if (type == 'function') {
            const name = val.name;
            if (typeof name == 'string' && name.length > 0) {
                return `Function(${name})`;
            } else {
                return 'Function';
            }
        }
        // objects
        if (Array.isArray(val)) {
            const length = val.length;
            let debug = '[';
            if (length > 0) {
                debug += debugString(val[0]);
            }
            for(let i = 1; i < length; i++) {
                debug += ', ' + debugString(val[i]);
            }
            debug += ']';
            return debug;
        }
        // Test for built-in
        const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
        let className;
        if (builtInMatches && builtInMatches.length > 1) {
            className = builtInMatches[1];
        } else {
            // Failed to match the standard '[object ClassName]'
            return toString.call(val);
        }
        if (className == 'Object') {
            // we're a user defined class or Object
            // JSON.stringify avoids problems with cycles, and is generally much
            // easier than looping through ownProperties of `val`.
            try {
                return 'Object(' + JSON.stringify(val) + ')';
            } catch (_) {
                return 'Object';
            }
        }
        // errors
        if (val instanceof Error) {
            return `${val.name}: ${val.message}\n${val.stack}`;
        }
        // TODO we could test for more things here, like `Set`s and `Map`s.
        return className;
    }

    function dropObject(idx) {
        if (idx < 1028) return;
        heap[idx] = heap_next;
        heap_next = idx;
    }

    function getArrayU8FromWasm0(ptr, len) {
        ptr = ptr >>> 0;
        return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
    }

    let cachedDataViewMemory0 = null;
    function getDataViewMemory0() {
        if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
            cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
        }
        return cachedDataViewMemory0;
    }

    function getStringFromWasm0(ptr, len) {
        return decodeText(ptr >>> 0, len);
    }

    let cachedUint8ArrayMemory0 = null;
    function getUint8ArrayMemory0() {
        if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
            cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
        }
        return cachedUint8ArrayMemory0;
    }

    function getObject(idx) { return heap[idx]; }

    function handleError(f, args) {
        try {
            return f.apply(this, args);
        } catch (e) {
            wasm.__wbindgen_export3(addHeapObject(e));
        }
    }

    let heap = new Array(1024).fill(undefined);
    heap.push(undefined, null, true, false);

    let heap_next = heap.length;

    function isLikeNone(x) {
        return x === undefined || x === null;
    }

    function makeClosure(arg0, arg1, f) {
        const state = { a: arg0, b: arg1, cnt: 1 };
        const real = (...args) => {

            // First up with a closure we increment the internal reference
            // count. This ensures that the Rust closure environment won't
            // be deallocated while we're invoking it.
            state.cnt++;
            try {
                return f(state.a, state.b, ...args);
            } finally {
                real._wbg_cb_unref();
            }
        };
        real._wbg_cb_unref = () => {
            if (--state.cnt === 0) {
                wasm.__wbindgen_export5(state.a, state.b);
                state.a = 0;
                CLOSURE_DTORS.unregister(state);
            }
        };
        CLOSURE_DTORS.register(real, state, state);
        return real;
    }

    function makeMutClosure(arg0, arg1, f) {
        const state = { a: arg0, b: arg1, cnt: 1 };
        const real = (...args) => {

            // First up with a closure we increment the internal reference
            // count. This ensures that the Rust closure environment won't
            // be deallocated while we're invoking it.
            state.cnt++;
            const a = state.a;
            state.a = 0;
            try {
                return f(a, state.b, ...args);
            } finally {
                state.a = a;
                real._wbg_cb_unref();
            }
        };
        real._wbg_cb_unref = () => {
            if (--state.cnt === 0) {
                wasm.__wbindgen_export5(state.a, state.b);
                state.a = 0;
                CLOSURE_DTORS.unregister(state);
            }
        };
        CLOSURE_DTORS.register(real, state, state);
        return real;
    }

    function passStringToWasm0(arg, malloc, realloc) {
        if (realloc === undefined) {
            const buf = cachedTextEncoder.encode(arg);
            const ptr = malloc(buf.length, 1) >>> 0;
            getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
            WASM_VECTOR_LEN = buf.length;
            return ptr;
        }

        let len = arg.length;
        let ptr = malloc(len, 1) >>> 0;

        const mem = getUint8ArrayMemory0();

        let offset = 0;

        for (; offset < len; offset++) {
            const code = arg.charCodeAt(offset);
            if (code > 0x7F) break;
            mem[ptr + offset] = code;
        }
        if (offset !== len) {
            if (offset !== 0) {
                arg = arg.slice(offset);
            }
            ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
            const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
            const ret = cachedTextEncoder.encodeInto(arg, view);

            offset += ret.written;
            ptr = realloc(ptr, len, offset, 1) >>> 0;
        }

        WASM_VECTOR_LEN = offset;
        return ptr;
    }

    function takeObject(idx) {
        const ret = getObject(idx);
        dropObject(idx);
        return ret;
    }

    let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
    cachedTextDecoder.decode();
    function decodeText(ptr, len) {
        return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
    }

    const cachedTextEncoder = new TextEncoder();

    if (!('encodeInto' in cachedTextEncoder)) {
        cachedTextEncoder.encodeInto = function (arg, view) {
            const buf = cachedTextEncoder.encode(arg);
            view.set(buf);
            return {
                read: arg.length,
                written: buf.length
            };
        };
    }

    let WASM_VECTOR_LEN = 0;

    let wasmModule, wasmInstance, wasm;
    function __wbg_finalize_init(instance, module) {
        wasmInstance = instance;
        wasm = instance.exports;
        wasmModule = module;
        cachedDataViewMemory0 = null;
        cachedUint8ArrayMemory0 = null;
        wasm.__wbindgen_start();
        return wasm;
    }

    async function __wbg_load(module, imports) {
        if (typeof Response === 'function' && module instanceof Response) {
            if (!module.ok) {
                throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
            }

            if (typeof WebAssembly.instantiateStreaming === 'function') {
                try {
                    return await WebAssembly.instantiateStreaming(module, imports);
                } catch (e) {
                    const validResponse = expectedResponseType(module.type);

                    if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                        console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                    } else { throw e; }
                }
            }

            const bytes = await module.arrayBuffer();
            return await WebAssembly.instantiate(bytes, imports);
        } else {
            const instance = await WebAssembly.instantiate(module, imports);

            if (instance instanceof WebAssembly.Instance) {
                return { instance, module };
            } else {
                return instance;
            }
        }

        function expectedResponseType(type) {
            switch (type) {
                case 'basic': case 'cors': case 'default': return true;
            }
            return false;
        }
    }

    function initSync(module) {
        if (wasm !== undefined) return wasm;


        if (module !== undefined) {
            if (Object.getPrototypeOf(module) === Object.prototype) {
                ({module} = module)
            } else {
                console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
            }
        }

        const imports = __wbg_get_imports();
        if (!(module instanceof WebAssembly.Module)) {
            module = new WebAssembly.Module(module);
        }
        const instance = new WebAssembly.Instance(module, imports);
        return __wbg_finalize_init(instance, module);
    }

    async function __wbg_init(module_or_path) {
        if (wasm !== undefined) return wasm;


        if (module_or_path !== undefined) {
            if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
                ({module_or_path} = module_or_path)
            } else {
                console.warn('using deprecated parameters for the initialization function; pass a single object instead')
            }
        }

        if (module_or_path === undefined && script_src !== undefined) {
            module_or_path = script_src.replace(/\.js$/, "_bg.wasm");
        }
        const imports = __wbg_get_imports();

        if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
            module_or_path = fetch(module_or_path);
        }

        const { instance, module } = await __wbg_load(await module_or_path, imports);

        return __wbg_finalize_init(instance, module);
    }

    return Object.assign(__wbg_init, { initSync }, exports);
})({ __proto__: null });
ftmlViewer();
