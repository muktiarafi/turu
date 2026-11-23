package com.muktiarafi.turu

import com.muktiarafi.turu.models.Api
import com.muktiarafi.turu.models.ScriptsView
import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The generated types (docs/NATIVE_UI_PROPOSAL.md, L4) against the views Rust writes: each model's
 * view as a fresh tab answers `GET /models/<name>` (`turu-typegen --samples`, run by the build
 * beside the types), decoded as [ModelClient] decodes it. A field renamed or retyped on one side
 * fails here, at build time, rather than on a page.
 */
class GeneratedTypesTest {
    /** As [ModelClient]: a field Rust adds that this build doesn't know isn't an error. */
    private val json = Json { ignoreUnknownKeys = true }

    private val samples = File(System.getProperty("turu.samples") ?: error("no turu.samples: run through Gradle"))

    private fun sample(name: String): JsonObject =
        json.parseToJsonElement(File(samples, "$name.json").readText()).jsonObject

    @Test
    fun scriptsViewDecodes() {
        val copy = sample(Api.scripts.name)
        assertTrue(copy["revision"]!!.jsonPrimitive.content.isNotEmpty())
        val view: ScriptsView = json.decodeFromJsonElement(Api.scripts.view!!, copy["view"]!!)
        // A fresh tab: nothing loaded.
        assertEquals(view, json.decodeFromJsonElement(ScriptsView.serializer(), copy["view"]!!))
    }

    @Test
    fun everyModelsViewDecodes() {
        val failed = mutableListOf<String>()
        var decoded = 0
        for (api in Api.all) {
            val strategy = api.view ?: continue
            val file = File(samples, "${api.name}.json")
            if (!file.exists()) {
                failed += "${api.name}: no sample"
                continue
            }
            runCatching { json.decodeFromJsonElement(strategy, sample(api.name)["view"]!!) }
                .onSuccess { decoded++ }
                .onFailure { failed += "${api.name}: ${it.message}" }
        }
        assertTrue(failed.joinToString("\n"), failed.isEmpty())
        assertEquals("a sample for every model with a view", samples.listFiles()!!.count { it.extension == "json" }, decoded)
    }
}
